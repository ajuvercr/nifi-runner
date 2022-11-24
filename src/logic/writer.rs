use std::collections::HashMap;

use oxigraph::{
    model::{GraphNameRef, Literal, NamedNodeRef, QuadRef, Term},
    sparql::QuerySolution,
    store::Store,
};

use crate::{
    client::{Nifi, PortType},
    models::{Component, ConnectionEntity, PortDTO, ProcessorDTO},
    sparql::{execute_query, get_parameter_solutions, NifiLinkQueryOutput, Queryable, WithSubject},
};

use super::{as_subject_ref, import_file_to_store, ID_TERM};

const WS_ONTOLOGY: &'static str = "./channels/ws_ontology.ttl";
const WS_WRITER_TEMPLATE: &'static str = "./channels/WSWriter.xml";

pub fn append_ontology(store: &oxigraph::store::Store) -> std::io::Result<()> {
    import_file_to_store(WS_ONTOLOGY, store)
}

async fn template_file_id(client: &Nifi, location: &str) -> Option<String> {
    let content = std::fs::read_to_string(location).ok()?;
    client.upload_template(content).await.ok()
}

pub async fn add_channel_writer(
    client: &crate::client::Nifi,
    store: &oxigraph::store::Store,
    procs: &HashMap<String, Component<ProcessorDTO>>,
) {
    let mut templates: HashMap<String, String> = HashMap::new();
    let mut ports: HashMap<String, Component<PortDTO>> = HashMap::new();

    let sols = get_parameter_solutions::<WriterQuery>(&store);

    for sol in sols.into_values() {
        println!("Creating writer");
        create_writer(client, store, sol, &mut templates, &mut ports).await;
    }

    let links = execute_query::<WriterLink>(store);
    for link in links {
        add_link(&client, link, &ports, procs).await;
    }

    for v in templates.into_values() {
        client.delete_template(&v).await.unwrap();
    }
}

fn unwr_lit(term: Term) -> String {
    match term {
        Term::Literal(l) => l.destruct().0,
        _ => panic!("Not a literal"),
    }
}

async fn create_writer(
    client: &Nifi,
    store: &Store,
    sol: Vec<QuerySolutionOutput>,
    templates: &mut HashMap<String, String>,
    port_map: &mut HashMap<String, Component<PortDTO>>,
) -> Option<()> {
    let writer_type = match &sol[0].writer_type {
        Term::NamedNode(n) => n.as_str(),
        _ => return None,
    };

    if !templates.contains_key(writer_type) {
        println!("Uploading new template");
        let location = match writer_type {
            "https://w3id.org/conn#WsWriterChannel" => WS_WRITER_TEMPLATE,
            _ => return None,
        };
        let template_id = template_file_id(client, location).await?;

        templates.insert(writer_type.to_string(), template_id);
    }

    println!("Creating writer");
    let template_id = templates.get(writer_type).unwrap();

    let flow = client.instantiate_template(template_id).await.ok()?;

    let group_client = client.change_group(&flow.flow.process_groups[0].id);
    let ports = group_client.get_ports(PortType::Input).await.ok()?.ports;
    let input_port = ports.get(0)?;
    port_map.insert(input_port.id.clone(), input_port.component.clone());

    let v = Literal::new_simple_literal(&input_port.id);
    store
        .insert(QuadRef {
            subject: as_subject_ref(sol[0].subject.as_ref()),
            predicate: NamedNodeRef::new(ID_TERM).unwrap().into(),
            object: v.as_ref().into(),
            graph_name: GraphNameRef::DefaultGraph,
        })
        .unwrap();

    let vars = sol
        .into_iter()
        .map(|sol| (unwr_lit(sol.nifi_key), unwr_lit(sol.value)));

    group_client.set_variables(vars).await.ok()?;

    Some(())
}

async fn add_link(
    client: &Nifi,
    NifiLinkQueryOutput {
        source_id,
        target_id,
        key,
    }: NifiLinkQueryOutput,
    ports: &HashMap<String, Component<PortDTO>>,
    procs: &HashMap<String, Component<ProcessorDTO>>,
) -> Option<()> {
    println!("adding link to writer");
    let source = procs.get(&source_id)?;
    let target = ports.get(&target_id)?;

    let body = ConnectionEntity::new(&source, &target, Some(&key));

    client
        .create_conection(body)
        .await
        .expect("Creating nifi connection");

    Some(())
}

#[derive(Clone, Debug)]
struct QuerySolutionOutput {
    pub subject: Term,

    pub nifi_key: Term,
    pub value: Term,
    pub writer_type: Term,
}

impl WithSubject for QuerySolutionOutput {
    fn subject(&self) -> &Term {
        &self.subject
    }
}

struct WriterQuery;
impl Queryable for WriterQuery {
    const ERROR: &'static str = "WS Writer query";

    const QUERY: &'static str = r#"
BASE <http://example.com/ns#>
PREFIX nifi: <https://w3id.org/conn/nifi#>
PREFIX sh: <http://www.w3.org/ns/shacl#>
PREFIX : <https://w3id.org/conn#> 
                
SELECT DISTINCT ?writer_type ?subject ?nifi_key ?value WHERE {
    ?sourceTy a <NifiProcess>;
      :shape [
        sh:property [
          sh:class :WriterChannel;
          sh:path ?sourcePath;
          nifi:key ?writerKey;
        ]
      ].

     _:source a ?sourceTy;
         ?sourcePath ?subject.

    ?writer_type :shape [
      sh:property [
        sh:path ?p;
        nifi:key ?nifi_key;
      ]
    ].

    ?subject a ?writer_type;
      ?p ?value.
}
"#;

    type Output = QuerySolutionOutput;
}

macro_rules! name {
    ($sol:ident, $key:ident) => {
        $sol.get(stringify!($key)).ok_or(stringify!($key))
    };
}
impl TryFrom<QuerySolution> for QuerySolutionOutput {
    type Error = &'static str;

    fn try_from(value: QuerySolution) -> Result<Self, Self::Error> {
        Ok(Self {
            subject: name!(value, subject)?.to_owned(),
            nifi_key: name!(value, nifi_key)?.to_owned(),
            value: name!(value, value)?.to_owned(),
            writer_type: name!(value, writer_type)?.to_owned(),
        })
    }
}

struct WriterLink;
impl Queryable for WriterLink {
    const ERROR: &'static str = "WS writer link";

    const QUERY: &'static str = r#"
BASE <http://example.com/ns#>
PREFIX nifi: <https://w3id.org/conn/nifi#>
PREFIX sh: <http://www.w3.org/ns/shacl#>
PREFIX : <https://w3id.org/conn#> 

SELECT * {
    ?sourceTy a <NifiProcess>;
      :shape [
        sh:property [
          sh:class :WriterChannel;
          sh:path ?sourcePath;
          nifi:key ?key;
        ]
      ].
    
     _:source a ?sourceTy;
       <http://example.com/ns#testing+id> ?source_id;
       ?sourcePath ?subject.

    ?subject <http://example.com/ns#testing+id> ?target_id.
}

"#;

    type Output = NifiLinkQueryOutput;
}
