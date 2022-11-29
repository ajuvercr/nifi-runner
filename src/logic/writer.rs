use std::{collections::HashMap, ops::Deref};

use derive::Query;
use oxigraph::{
    model::{GraphNameRef, Literal, NamedNodeRef, QuadRef, Term},
    store::Store,
};

use super::{as_subject_ref, import_file_to_store, ID_TERM};
use crate::{
    client::{Nifi, PortType},
    logic::template_file_id,
    models::{Component, ConnectionEntity, PortDTO, ProcessorDTO},
    sparql::{
        execute_query, get_parameter_solutions, NifiLinkQueryOutput, QueryField, QueryString,
        Queryable, Sol, WithSubject,
    },
};

static WRITERS: &[(&'static str, &'static str)] = &[(
    "https://w3id.org/conn#WsWriterChannel",
    "./channels/WSWriter.xml",
)];

const WS_ONTOLOGY: &'static str = "./channels/ws_ontology.ttl";

pub fn append_ontology(store: &oxigraph::store::Store) -> std::io::Result<()> {
    import_file_to_store(WS_ONTOLOGY, store)
}

pub async fn add_channel_writer(
    client: &crate::client::Nifi,
    store: &oxigraph::store::Store,
    procs: &HashMap<String, Component<ProcessorDTO>>,
    start: bool,
) {
    let mut templates: HashMap<String, String> = HashMap::new();
    let mut ports: HashMap<String, Component<PortDTO>> = HashMap::new();

    let sols = get_parameter_solutions::<WriterQuery>(&store);
    let mut clients = Vec::new();

    for sol in sols.into_values() {
        println!("Creating writer");

        if let Some(client) = create_writer(client, store, sol, &mut templates, &mut ports).await {
            clients.push(client);
        } else {
            eprintln!("Failed to add writer!");
        }
    }

    let links = execute_query::<WriterLink>(store);
    for link in links {
        add_link(&client, link, &ports, procs).await;
    }

    for v in templates.into_values() {
        client.delete_template(&v).await.unwrap();
    }

    if start {
        for client in clients {
            if let Err(e) = client.start_process_group().await {
                eprintln!("Failed to start process group\n{:?}", e);
            }
        }
    }
}

async fn create_writer(
    client: &Nifi,
    store: &Store,
    sol: Vec<QuerySolutionOutput>,
    templates: &mut HashMap<String, String>,
    port_map: &mut HashMap<String, Component<PortDTO>>,
) -> Option<Nifi> {
    let writer_type = match &sol[0].writer_type.0 {
        Term::NamedNode(n) => n.as_str(),
        _ => return None,
    };

    if !templates.contains_key(writer_type) {
        println!("Uploading new template");
        let location = WRITERS.iter().find(|x| x.0 == writer_type).map(|x| x.1)?;

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
            subject: as_subject_ref(sol[0].subject.0.as_ref()),
            predicate: NamedNodeRef::new(ID_TERM).unwrap().into(),
            object: v.as_ref().into(),
            graph_name: GraphNameRef::DefaultGraph,
        })
        .unwrap();

    let vars = sol.into_iter().map(|sol| (sol.nifi_key.0, sol.value.0));

    group_client.set_variables(vars).await.ok()?;

    Some(group_client)
}

async fn add_link<'a>(
    client: &'a Nifi,
    NifiLinkQueryOutput {
        source_id,
        target_id,
        key,
    }: NifiLinkQueryOutput<QueryString<"key">>,
    ports: &'a HashMap<String, Component<PortDTO>>,
    procs: &'a HashMap<String, Component<ProcessorDTO>>,
) -> Option<()> {
    println!("adding link to writer");
    let source = procs.get(source_id.deref())?;
    let target = ports.get(target_id.deref())?;

    let body = ConnectionEntity::new(&source, &target, Some(&key));

    client
        .create_conection(body)
        .await
        .expect("Creating nifi connection");

    Some(())
}

#[derive(Clone, Debug, Query)]
struct QuerySolutionOutput {
    pub subject: QueryField<Term, "subject">,
    pub nifi_key: QueryField<String, "nifi_key">,
    pub value: QueryField<String, "value">,
    pub writer_type: QueryField<Term, "writer_type">,
}

impl WithSubject for QuerySolutionOutput {
    fn subject(&self) -> &Term {
        &self.subject
    }
}

struct WriterQuery;
impl Queryable for WriterQuery {
    type Output = QuerySolutionOutput;
    const ERROR: &'static str = "WS Writer query";

    const QUERY: &'static str = r#"
PREFIX nifi: <https://w3id.org/conn/nifi#>
PREFIX sh: <http://www.w3.org/ns/shacl#>
PREFIX : <https://w3id.org/conn#> 
                
SELECT DISTINCT ?writer_type ?subject ?nifi_key ?value WHERE {
    ?sourceTy a nifi:NifiProcess.

    [] sh:targetClass ?sourceTy;
        sh:property [
          sh:class :WriterChannel;
          sh:path ?sourcePath;
          nifi:key ?writerKey;
        ].

     _:source a ?sourceTy;
         ?sourcePath ?subject.

    [] sh:targetClass ?writer_type;
        sh:property [
          sh:path ?p;
          nifi:key ?nifi_key;
        ].

    ?subject a ?writer_type;
      ?p ?value.
}
"#;
}

struct WriterLink;
impl Queryable for WriterLink {
    const ERROR: &'static str = "WS writer link";

    const QUERY: &'static str = r#"
PREFIX nifi: <https://w3id.org/conn/nifi#>
PREFIX sh: <http://www.w3.org/ns/shacl#>
PREFIX : <https://w3id.org/conn#> 

SELECT * {
    ?sourceTy a nifi:NifiProcess.
    [] sh:targetClass ?sourceTy;
        sh:property [
          sh:class :WriterChannel;
          sh:path ?sourcePath;
          nifi:key ?key;
        ].
    
     _:source a ?sourceTy;
       <http://example.com/ns#testing+id> ?source_id;
       ?sourcePath ?subject.

    ?subject <http://example.com/ns#testing+id> ?target_id.
}
"#;

    type Output = NifiLinkQueryOutput<QueryString<"key">>;
}
