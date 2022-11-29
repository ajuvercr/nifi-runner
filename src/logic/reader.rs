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
        execute_query, get_parameter_solutions, NifiLinkQueryOutput, QueryField, Queryable, Sol,
        WithSubject,
    },
};

const WS_ONTOLOGY: &'static str = "./channels/ws_ontology.ttl";
static READERS: &[(&'static str, &'static str)] = &[(
    "https://w3id.org/conn#WsReaderChannel",
    "./channels/WSReader.xml",
)];

pub fn append_ontology(store: &oxigraph::store::Store) -> std::io::Result<()> {
    import_file_to_store(WS_ONTOLOGY, store)
}

pub async fn add_channel_reader(
    client: &crate::client::Nifi,
    store: &oxigraph::store::Store,
    procs: &HashMap<String, Component<ProcessorDTO>>,
) {
    let mut templates: HashMap<String, String> = HashMap::new();
    let mut ports: HashMap<String, Component<PortDTO>> = HashMap::new();

    let sols = get_parameter_solutions::<ReaderQuery>(&store);
    let mut clients = Vec::new();

    for sol in sols.into_values() {
        if let Some(client) = create_reader(client, store, sol, &mut templates, &mut ports).await {
            clients.push(client);
        } else {
            eprintln!("Failed to add reader");
        }
    }

    let links = execute_query::<ReaderLink>(store);
    for link in links {
        if add_link(&client, link, &ports, procs).await.is_none() {
            eprintln!("Failed to add link");
        }
    }

    for v in templates.into_values() {
        if let Err(e) = client.delete_template(&v).await {
            eprintln!("Failed to delete template {:?}", e.error_kind());
        }
    }

    for client in clients {
        if let Err(e) = client.start_process_group().await {
            eprintln!("Failed to start process group\n{:?}", e);
        }
    }
}

async fn create_reader(
    client: &Nifi,
    store: &Store,
    sol: Vec<QuerySolutionOutput>,
    templates: &mut HashMap<String, String>,
    port_map: &mut HashMap<String, Component<PortDTO>>,
) -> Option<Nifi> {
    println!("Creating reader");
    let reader_type = match &sol[0].reader_type.0 {
        Term::NamedNode(n) => n.as_str(),
        _ => {
            println!("expected named node");
            return None;
        }
    };

    if !templates.contains_key(reader_type) {
        println!("Uploading new template");
        let location = READERS.iter().find(|x| x.0 == reader_type).map(|x| x.1)?;

        let template_id = template_file_id(client, location).await?;

        templates.insert(reader_type.to_string(), template_id);
    }

    let template_id = templates.get(reader_type).unwrap();
    println!("Found template");

    let flow = match client.instantiate_template(template_id).await {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Failed to instantiate template\n{:?}", e);
            return None;
        }
    };

    let group_client = client.change_group(&flow.flow.process_groups[0].id);

    let ports = group_client.get_ports(PortType::Output).await.ok()?.ports;
    let output_port = ports.get(0)?;
    port_map.insert(output_port.id.clone(), output_port.component.clone());

    let v = Literal::new_simple_literal(&output_port.id);
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

async fn add_link(
    client: &Nifi,
    NifiLinkQueryOutput {
        source_id,
        target_id,
        ..
    }: NifiLinkQueryOutput<()>,
    ports: &HashMap<String, Component<PortDTO>>,
    procs: &HashMap<String, Component<ProcessorDTO>>,
) -> Option<()> {
    println!("adding link to reader");
    let source = ports.get(source_id.deref())?;
    let target = procs.get(target_id.deref())?;

    let body = ConnectionEntity::new(&source, &target, None);

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
    pub reader_type: QueryField<Term, "reader_type">,
}

impl WithSubject for QuerySolutionOutput {
    fn subject(&self) -> &Term {
        &self.subject
    }
}

struct ReaderQuery;

impl Queryable for ReaderQuery {
    type Output = QuerySolutionOutput;
    const ERROR: &'static str = "WS Reader query";

    const QUERY: &'static str = r#"
BASE <http://example.com/ns#>
PREFIX nifi: <https://w3id.org/conn/nifi#>
PREFIX sh: <http://www.w3.org/ns/shacl#>
PREFIX : <https://w3id.org/conn#> 
                
SELECT DISTINCT ?reader_type ?subject ?nifi_key ?value WHERE {
    ?sourceTy a <NifiProcess>;
      :shape [
        sh:property [
          sh:class :ReaderChannel;
          sh:path ?sourcePath;
        ]
      ].

     _:source a ?sourceTy;
         ?sourcePath ?subject.

    ?reader_type :shape [
      sh:property [
        sh:path ?p;
        nifi:key ?nifi_key;
      ]
    ].

    ?subject a ?reader_type;
      ?p ?value.
}
"#;
}

struct ReaderLink;
impl Queryable for ReaderLink {
    const ERROR: &'static str = "WS reader link";

    const QUERY: &'static str = r#"
BASE <http://example.com/ns#>
PREFIX nifi: <https://w3id.org/conn/nifi#>
PREFIX sh: <http://www.w3.org/ns/shacl#>
PREFIX : <https://w3id.org/conn#> 

SELECT * {
    ?sourceTy a <NifiProcess>;
      :shape [
        sh:property [
          sh:class :ReaderChannel;
          sh:path ?sourcePath;
        ]
      ].
    
     _:source a ?sourceTy;
       <http://example.com/ns#testing+id> ?target_id;
       ?sourcePath ?subject.

    ?subject <http://example.com/ns#testing+id> ?source_id.
}
"#;

    type Output = NifiLinkQueryOutput<()>;
}
