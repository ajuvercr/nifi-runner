use std::{collections::HashMap, ops::Deref};

use derive::Query;
use oxigraph::{
    model::Term,
    sparql::{QueryResults, QuerySolution},
    store::Store,
};

#[derive(Clone, Copy)]
pub struct Sol<'a>(&'a QuerySolution);

impl<'a> Deref for Sol<'a> {
    type Target = QuerySolution;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub trait WithSubject {
    fn subject(&self) -> &Term;
}

pub trait FromQuery: Sized {
    fn from_query(sol: Sol) -> Result<Self, &'static str>;
}

impl FromQuery for () {
    fn from_query(_: Sol) -> Result<Self, &'static str> {
        Ok(())
    }
}

impl<T> FromQuery for T
where
    T: for<'a> TryFrom<Sol<'a>, Error = &'static str>,
{
    fn from_query(sol: Sol) -> Result<Self, &'static str> {
        sol.try_into()
    }
}

pub trait Queryable {
    const ERROR: &'static str;
    const QUERY: &'static str;
    type Output;
}

#[derive(Clone, Debug)]
pub enum ShaclType {
    DataType(Term),
    Class(Term),
    None,
}

impl FromQuery for ShaclType {
    fn from_query(sol: Sol) -> Result<Self, &'static str> {
        Ok(match (sol.get("datatype"), sol.get("class")) {
            (None, None) => ShaclType::None,
            (Some(x), None) => ShaclType::DataType(x.to_owned()),
            (None, Some(x)) => ShaclType::Class(x.to_owned()),
            (Some(_), Some(_)) => return Err("Both datatype and class specified"),
        })
    }
}

impl<T> FromQuery for Option<T>
where
    T: FromQuery,
{
    fn from_query(sol: Sol) -> Result<Self, &'static str> {
        match T::from_query(sol) {
            Ok(x) => Ok(Some(x)),
            Err(_) => Ok(None),
        }
    }
}

#[derive(Clone, Debug, Query)]
pub struct QuerySolutionOutput {
    pub subject: QueryField<Term, "subject">,

    pub nifi_key: Option<QueryString<"nifi_key">>,
    pub value: QueryField<Term, "value">,
    pub shacl_type: ShaclType,
    pub ty: QueryString<"ty">,
}

pub struct ProcessorQuery;
impl Queryable for ProcessorQuery {
    const ERROR: &'static str = "Process Query";
    const QUERY: &'static str = r#" 
PREFIX nifi: <https://w3id.org/conn/nifi#>
PREFIX sh: <http://www.w3.org/ns/shacl#>
PREFIX fno: <https://w3id.org/function/ontology#>
PREFIX fnom: <https://w3id.org/function/vocabulary/mapping#>
PREFIX : <https://w3id.org/conn#> 
                
  SELECT * WHERE {

    ?tys a nifi:NifiProcess;
        nifi:type ?ty
        .

    ?shape sh:targetClass ?tys;
        sh:property [
          sh:path ?p;
        ].

    ?subject a ?tys;
      ?p ?value.

    OPTIONAL { ?shape sh:property [ sh:path ?p; sh:datatype ?datatype ] }
    OPTIONAL { ?shape sh:property [ sh:path ?p; sh:class ?class ] }
    OPTIONAL {
      ?tys nifi:mapping [
        fno:parameterMapping [
          fnom:functionParameter ?p;
          fnom:implementationParameterPosition ?nifi_key;
        ]
      ].
    }
  }
"#;

    type Output = QuerySolutionOutput;
}

impl WithSubject for QuerySolutionOutput {
    fn subject(&self) -> &Term {
        &self.subject
    }
}

type St = &'static str;

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub struct QueryField<T, const KEY: St>(pub T);
pub type QueryString<const KEY: St> = QueryField<String, KEY>;

impl<T, const KEY: &'static str> Deref for QueryField<T, KEY> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, const KEY: &'static str> AsRef<T> for QueryField<T, KEY> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

pub trait FromTerm: Sized {
    fn from_term(this: &Term) -> Result<Self, &'static str>;
}

impl FromTerm for Term {
    fn from_term(this: &Term) -> Result<Self, &'static str> {
        Ok(this.clone())
    }
}

impl FromTerm for String {
    fn from_term(this: &Term) -> Result<Self, &'static str> {
        match this {
            Term::NamedNode(_) => Err("Expected literal, found named node"),
            Term::BlankNode(_) => Err("Expected literal, found blank node"),
            Term::Literal(l) => Ok(l.value().to_string()),
            Term::Triple(_) => Err("Expected literal, found triple"),
        }
    }
}

impl<T, const KEY: &'static str> FromQuery for QueryField<T, KEY>
where
    T: FromTerm,
{
    fn from_query(sol: Sol) -> Result<Self, &'static str> {
        let v = sol.get(KEY).ok_or(KEY)?;
        Ok(Self(T::from_term(v)?))
    }
}

#[derive(Debug, Query)]
pub struct NifiLinkQueryOutput<Rest> {
    pub source_id: QueryString<"source_id">,
    pub target_id: QueryString<"target_id">,
    pub key: Rest,
}

pub struct NifiLinkQuery;
impl Queryable for NifiLinkQuery {
    const ERROR: &'static str = "Nifi link query";
    const QUERY: &'static str = r#"
PREFIX nifi: <https://w3id.org/conn/nifi#>
PREFIX sh: <http://www.w3.org/ns/shacl#>
PREFIX fno: <https://w3id.org/function/ontology#>
PREFIX fnom: <https://w3id.org/function/vocabulary/mapping#>
PREFIX : <https://w3id.org/conn#> 
                
SELECT * WHERE {
    _:channel a nifi:NifiChannel;
      :reader ?reader;
      :writer ?writer.

    ?sourceTy a nifi:NifiProcess.
    [] sh:targetClass ?sourceType;
       sh:property [
         sh:class :WriterChannel;
         sh:path ?sourcePath;
       ].

    ?sourceType nifi:mapping [
        fno:parameterMapping [
          fnom:functionParameter ?sourcePath;
          fnom:implementationParameterPosition ?key;
        ]
      ].

    ?targetTy a nifi:NifiProcess.

    [] sh:targetClass ?targetTy;
       sh:property [
         sh:class :ReaderChannel;
         sh:path ?targetPath;
       ].

   _:source a ?sourceTy;
     <http://example.com/ns#testing+id> ?source_id;
     ?sourcePath ?writer.
     
   _:target a ?targetTy;
     <http://example.com/ns#testing+id> ?target_id;
     ?targetPath ?reader.
}
"#;
    type Output = NifiLinkQueryOutput<QueryString<"key">>;
}

pub fn execute_query<T: Queryable>(store: &Store) -> Vec<T::Output>
where
    T::Output: FromQuery,
{
    println!("Exectuting query {}", stringify!(T));

    if let QueryResults::Solutions(solutions) = store.query(T::QUERY).unwrap() {
        solutions
            .flatten()
            .flat_map(|s| match T::Output::from_query(Sol(&s)) {
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
    T::Output: FromQuery,
    T::Output: WithSubject,
{
    let mut per_subject: HashMap<Term, Vec<T::Output>> = HashMap::new();

    if let QueryResults::Solutions(solutions) = store.query(T::QUERY).unwrap() {
        for sol in solutions.flatten() {
            let param = match T::Output::from_query(Sol(&sol)) {
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
