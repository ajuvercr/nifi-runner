use std::collections::HashMap;

use oxigraph::{
    model::Term,
    sparql::{QueryResults, QuerySolution},
    store::Store,
};

use crate::util::unwrap_literal;

macro_rules! name {
    ($sol:ident, $key:ident) => {
        $sol.get(stringify!($key)).ok_or(stringify!($key))
    };
}

pub trait WithSubject {
    fn subject(&self) -> &Term;
}

pub trait Queryable {
    const ERROR: &'static str;
    const QUERY: &'static str;
    type Output: TryFrom<QuerySolution, Error = &'static str>;
}

#[derive(Clone, Debug)]
pub enum ShaclType {
    DataType(Term),
    Class(Term),
    None,
}

#[derive(Clone, Debug)]
pub struct QuerySolutionOutput {
    pub subject: Term,

    pub nifi_key: Option<String>,
    pub value: Term,
    pub shacl_type: ShaclType,
    pub ty: String,
}

pub struct ProcessorQuery;
impl Queryable for ProcessorQuery {
    const ERROR: &'static str = "Process Query";
    const QUERY: &'static str = r#" 
BASE <http://example.com/ns#>
PREFIX nifi: <https://w3id.org/conn/nifi#>
PREFIX sh: <http://www.w3.org/ns/shacl#>
PREFIX : <https://w3id.org/conn#> 
                
  SELECT * WHERE {

    ?tys a <NifiProcess>;
      :processProperties [
        nifi:type ?ty
      ];
      :shape [
        sh:property [
          sh:path ?p;
        ];
      ].

    ?subject a ?tys;
      ?p ?value.

    OPTIONAL { ?tys :shape [ sh:property [ sh:path ?p; sh:datatype ?datatype]]}
    OPTIONAL { ?tys :shape [ sh:property [ sh:path ?p; sh:class ?class]]}
    OPTIONAL { ?tys :shape [ sh:property [ sh:path ?p; nifi:key ?nifi_key]]}
  }
"#;

    type Output = QuerySolutionOutput;
}

impl WithSubject for QuerySolutionOutput {
    fn subject(&self) -> &Term {
        &self.subject
    }
}

impl TryFrom<QuerySolution> for QuerySolutionOutput {
    type Error = &'static str;

    fn try_from(sol: QuerySolution) -> Result<Self, Self::Error> {
        let rest: HashMap<_, _> = sol
            .into_iter()
            .map(|(v, k)| (v.as_str().to_string(), k.to_owned()))
            .collect();

        let subject = name!(rest, subject)?.to_owned();

        let nifi_key = rest
            .get("nifi_key")
            .and_then(unwrap_literal)
            .map(String::from);

        let shacl_type = match (rest.get("datatype"), rest.get("class")) {
            (None, None) => ShaclType::None,
            (Some(x), None) => ShaclType::DataType(x.to_owned()),
            (None, Some(x)) => ShaclType::Class(x.to_owned()),
            (Some(_), Some(_)) => return Err("Both datatype and class specified"),
        };

        let ty = rest
            .get("ty")
            .and_then(unwrap_literal)
            .ok_or("ty")?
            .to_owned();

        let this = Self {
            nifi_key,
            value: name!(rest, value)?.to_owned(),
            subject: subject.clone(),

            shacl_type,
            ty,
        };

        Ok(this)
    }
}

#[derive(Debug)]
pub struct NifiLinkQueryOutput {
    pub source_id: String,
    pub target_id: String,
    pub key: String,
}

impl TryFrom<QuerySolution> for NifiLinkQueryOutput {
    type Error = &'static str;

    fn try_from(value: QuerySolution) -> Result<Self, Self::Error> {
        let source_id = value
            .get("source_id")
            .and_then(unwrap_literal)
            .map(String::from)
            .ok_or("source_id")?;

        let target_id = value
            .get("target_id")
            .and_then(unwrap_literal)
            .map(String::from)
            .ok_or("target_id")?;

        let key = value
            .get("key")
            .and_then(unwrap_literal)
            .map(String::from)
            .ok_or("key")?;

        Ok(Self {
            source_id,
            target_id,
            key,
        })
    }
}

pub struct NifiLinkQuery;
impl Queryable for NifiLinkQuery {
    const ERROR: &'static str = "Nifi link query";
    const QUERY: &'static str = r#"
BASE <http://example.com/ns#>
PREFIX nifi: <https://w3id.org/conn/nifi#>
PREFIX sh: <http://www.w3.org/ns/shacl#>
PREFIX : <https://w3id.org/conn#> 
                
SELECT * WHERE {
    _:channel a :NifiChannel;
      :reader ?reader;
      :writer ?writer.

    ?sourceTy a <NifiProcess>;
      :shape [
        sh:property [
          sh:class :WriterChannel;
          sh:path ?sourcePath;
          nifi:key ?key;
        ]
      ].

    ?targetTy a <NifiProcess>;
      :shape [
        sh:property [
          sh:class :ReaderChannel;
          sh:path ?targetPath;
        ]
      ].

   _:source a ?sourceTy;
     <http://example.com/ns#testing+id> ?source_id;
     ?sourcePath ?writer.
     
   _:target a ?targetTy;
     <http://example.com/ns#testing+id> ?target_id;
     ?targetPath ?reader.
}
"#;
    type Output = NifiLinkQueryOutput;
}

pub fn execute_query<T: Queryable>(store: &Store) -> Vec<T::Output> {
    if let QueryResults::Solutions(solutions) = store.query(T::QUERY).unwrap() {
        solutions
            .flatten()
            .flat_map(|s| match T::Output::try_from(s) {
                Ok(x) => Some(x),
                Err(e) => {
                    eprintln!("Failed to create {}: {}", T::ERROR, e);
                    None
                }
            })
            .collect()
    } else {
        Vec::new()
    }
}

pub fn get_parameter_solutions<T: Queryable>(store: &Store) -> HashMap<Term, Vec<T::Output>>
where
    T::Output: WithSubject,
{
    let mut per_subject: HashMap<Term, Vec<T::Output>> = HashMap::new();

    if let QueryResults::Solutions(solutions) = store.query(T::QUERY).unwrap() {
        for sol in solutions.flatten() {
            let param = match T::Output::try_from(sol) {
                Ok(x) => x,
                Err(x) => {
                    eprintln!("Failed to create solution parameter, {}", x);
                    continue;
                }
            };

            if let Some(x) = per_subject.get_mut(param.subject()) {
                x.push(param);
            } else {
                per_subject.insert(param.subject().clone(), vec![param]);
            }
        }
    }

    per_subject
}
