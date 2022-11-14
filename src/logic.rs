use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

use oxigraph::model::{
    BlankNode, GraphName, GraphNameRef, Literal, NamedNode, NamedNodeRef, Quad, QuadRef, Subject,
    SubjectRef, Term, TermRef, Triple,
};
use oxigraph::sparql::{QueryResults, QuerySolution};
use oxigraph::store::{BulkLoader, Store};
use rio_api::model as rm;
use rio_api::parser::TriplesParser;
use rio_turtle::{TurtleError, TurtleParser};

use crate::client;
use crate::models::ConnectionEntity;

// #[inline]
fn map_triple<'a>(sub: &'a rm::Triple<'a>) -> Triple {
    Triple::new(
        map_subject(&sub.subject),
        map_predicate(&sub.predicate),
        map_object(&sub.object),
    )
}

// #[inline]
fn map_subject<'a>(sub: &'a rm::Subject<'a>) -> Subject {
    match sub {
        rm::Subject::NamedNode(n) => Subject::NamedNode(NamedNode::new_unchecked(n.iri)),
        rm::Subject::BlankNode(n) => Subject::BlankNode(BlankNode::new_unchecked(n.id)),
        rm::Subject::Triple(n) => Subject::Triple(Box::new(map_triple(n))),
    }
}

// #[inline]
fn map_predicate<'a>(pred: &'a rm::NamedNode<'a>) -> NamedNode {
    NamedNode::new_unchecked(pred.iri)
}

// #[inline]
fn map_literal<'a>(lit: &'a rm::Literal<'a>) -> Literal {
    match lit {
        rm::Literal::Simple { value } => Literal::new_simple_literal(*value),
        rm::Literal::LanguageTaggedString { value, language } => {
            Literal::new_language_tagged_literal_unchecked(*value, *language)
        }
        rm::Literal::Typed { value, datatype } => {
            Literal::new_typed_literal(*value, map_predicate(datatype))
        }
    }
}

// #[inline]
fn map_object<'a>(object: &'a rm::Term<'a>) -> Term {
    match object {
        rm::Term::NamedNode(n) => Term::NamedNode(NamedNode::new_unchecked(n.iri)),
        rm::Term::BlankNode(n) => Term::BlankNode(BlankNode::new_unchecked(n.id)),
        rm::Term::Literal(n) => Term::Literal(map_literal(n)),
        rm::Term::Triple(n) => Term::Triple(Box::new(map_triple(n))),
    }
}

// #[inline]
fn map_triple_to_quad(
    rm::Triple {
        subject,
        predicate,
        object,
    }: rm::Triple,
) -> Quad {
    Quad::new(
        map_subject(&subject),
        map_predicate(&predicate),
        map_object(&object),
        GraphName::DefaultGraph,
    )
}

fn load_file_to_store<R: BufRead + Sized>(file: R, bl: &BulkLoader) -> std::io::Result<()> {
    let parser = TurtleParser::new(file, None);
    bl.load_quads(
        parser
            .into_iter::<_, TurtleError, _>(|triple| Ok(map_triple_to_quad(triple)))
            .flatten(),
    )
    .expect("bulk loading store");
    Ok(())
}

const ID_TERM: &str = "http://example.com/ns#testing+id";
fn as_subject_ref(t: TermRef) -> SubjectRef {
    match t {
        TermRef::NamedNode(n) => SubjectRef::NamedNode(n),
        TermRef::BlankNode(n) => SubjectRef::BlankNode(n),
        _ => panic!(),
    }
}

async fn create_foobar(solution: Vec<QuerySolution>, store: &Store) {
    let ty = solution[0].get("ty").unwrap();
    let mut proc = client::Nifi::new_processor("root", unwrap_literal(ty).unwrap())
        .await
        .expect("Nifi failed me again");

    let v = Literal::new_simple_literal(&proc.id);
    store
        .insert(QuadRef {
            subject: as_subject_ref(solution[0].get("s").unwrap().as_ref().into()),
            predicate: NamedNodeRef::new(ID_TERM).unwrap().into(),
            object: v.as_ref().into(),
            graph_name: GraphNameRef::DefaultGraph,
        })
        .unwrap();

    for sol in solution {
        if sol.get("dt2").is_some() {
            continue;
        }
        let key = sol.get("key").and_then(unwrap_literal).unwrap().to_string();
        let object = sol.get("o").and_then(unwrap_literal).unwrap().to_string();

        proc.component.config.properties.insert(key, Some(object));
    }

    println!("Updating processor");
    client::Nifi::update_prcocessor(&proc.id, &proc)
        .await
        .expect("Nifi failed med");
    println!("Updated processor");
}

const QUERY: &str = r#" 
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

    ?s a ?tys;
      ?p ?o.

    OPTIONAL { ?tys :shape [ sh:property [ sh:path ?p; sh:datatype ?dt]]}
    OPTIONAL { ?tys :shape [ sh:property [ sh:path ?p; sh:class ?dt2]]}
    OPTIONAL { ?tys :shape [ sh:property [ sh:path ?p; nifi:key ?key]]}
  }
"#;

const CHANNEL_QUERY: &str = r#"
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
     <http://example.com/ns#testing+id> ?sourceId;
     ?sourcePath ?writer.
     
   _:target a ?targetTy;
     <http://example.com/ns#testing+id> ?targetId;
     ?targetPath ?reader.
}
"#;

fn unwrap_literal<'a>(x: &'a Term) -> Option<&str> {
    match x {
        Term::Literal(l) => Some(l.value()),
        _ => None,
    }
}

fn unwrap_node<'a>(x: Option<&'a Term>) -> Option<&'a str> {
    match x {
        Some(Term::BlankNode(n)) => Some(n.as_str()),
        Some(Term::NamedNode(n)) => Some(n.as_str()),
        _ => None,
    }
}

pub async fn startup(ontology: String, input: Option<String>) {
    let store = Store::new().unwrap();

    let bl = store.bulk_loader();
    if let Some(input) = input {
        println!("Loading files {}", input);
        load_file_to_store(BufReader::new(File::open(input).unwrap()), &bl)
            .expect("Import in store");
    } else {
        println!("Loading files stdin");
        load_file_to_store(std::io::stdin().lock(), &bl).expect("Import in store");
    }

    println!("Loaded ontology {}", ontology);
    load_file_to_store(BufReader::new(File::open(ontology).unwrap()), &bl)
        .expect("Load file to store");

    // SPARQL query
    if let QueryResults::Solutions(solutions) = store.query(QUERY).unwrap() {
        let sol: Vec<_> = solutions.flatten().collect();

        let mut per_subject: HashMap<String, Vec<QuerySolution>> = HashMap::new();

        for sol in sol {
            let id = unwrap_node(sol.get("s")).unwrap();

            if let Some(x) = per_subject.get_mut(id) {
                x.push(sol);
            } else {
                per_subject.insert(id.to_string(), vec![sol]);
            }
        }

        for v in per_subject.into_values() {
            create_foobar(v, &store).await;
        }
    }

    if let QueryResults::Solutions(solutions) = store.query(CHANNEL_QUERY).unwrap() {
        for sol in solutions.flatten() {
            add_nifi_link(sol).await;
        }
    }
}

async fn add_nifi_link(sol: QuerySolution) -> Option<()> {
    println!("Found a solution");
    let source_id = unwrap_literal(sol.get("sourceId")?)?;
    let target_id = unwrap_literal(sol.get("targetId")?)?;
    let key = unwrap_literal(sol.get("key")?)?;

    println!("source {} key {} target {}", source_id, key, target_id);

    let body = ConnectionEntity::new(source_id, target_id, key);
    client::Nifi::create_conection("root", body)
        .await
        .expect("Creating nifi connection");

    Some(())
}
