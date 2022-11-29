use std::collections::HashMap;

use crate::logic::reader::{self, add_channel_reader};
use crate::logic::writer::{self, add_channel_writer};
use crate::logic::{import_file_to_store, import_reader_to_store};
use crate::sparql::{
    execute_query, get_parameter_solutions, NifiLinkQuery, NifiLinkQueryOutput, ProcessorQuery,
    QuerySolutionOutput, QueryString, ShaclType,
};
use oxigraph::model::{GraphNameRef, Literal, NamedNodeRef, QuadRef, SubjectRef, Term, TermRef};
use oxigraph::store::Store;

use crate::client::Nifi;
use crate::models::{Component, ConnectionEntity, ProcessorDTO};

pub const ID_TERM: &str = "http://example.com/ns#testing+id";

pub async fn startup(client: Nifi, ontology: String, input: Option<String>, start: bool) {
    let store = Store::new().unwrap();

    if let Some(input) = input {
        println!("Loading files {}", input);
        import_file_to_store(input, &store).expect("Load file to store");
    } else {
        println!("Loading files stdin");
        import_reader_to_store(std::io::stdin().lock(), &store).expect("Import in store");
    }

    println!("Loaded ontology {}", ontology);
    import_file_to_store(ontology, &store).expect("Load file to store");

    writer::append_ontology(&store).expect("Loading WS writer ontology");
    reader::append_ontology(&store).expect("Loading WS reader ontology");

    let mut create_processors = HashMap::new();

    let per_subject = get_parameter_solutions::<ProcessorQuery>(&store);

    for v in per_subject.into_values() {
        let (id, proc) = create_processor(&client, v, &store).await;
        create_processors.insert(id, proc);
    }

    for link in execute_query::<NifiLinkQuery>(&store) {
        add_nifi_link(&client, link, &create_processors).await;
    }

    add_channel_writer(&client, &store, &create_processors, start).await;
    add_channel_reader(&client, &store, &create_processors, start).await;

    if start {
        for proc in create_processors.into_values() {
            if let Err(e) = client.start_processor(&proc.id).await {
                println!("Start processor failed\n{:?}", e);
            }
        }
    }
}

pub fn as_subject_ref(t: TermRef) -> SubjectRef {
    match t {
        TermRef::NamedNode(n) => SubjectRef::NamedNode(n),
        TermRef::BlankNode(n) => SubjectRef::BlankNode(n),
        _ => panic!(),
    }
}

async fn create_processor(
    client: &Nifi,
    solution: Vec<QuerySolutionOutput>,
    store: &Store,
) -> (String, Component<ProcessorDTO>) {
    println!("Creating processor {:?}", solution[0].ty);
    let mut proc = client
        .new_processor(&solution[0].ty)
        .await
        .expect("Nifi failed me again");

    let v = Literal::new_simple_literal(&proc.id);

    store
        .insert(QuadRef {
            subject: as_subject_ref(solution[0].subject.0.as_ref()),
            predicate: NamedNodeRef::new(ID_TERM).unwrap().into(),
            object: v.as_ref().into(),
            graph_name: GraphNameRef::DefaultGraph,
        })
        .unwrap();

    for sol in solution {
        if let ShaclType::Class(_) = sol.shacl_type {
            continue;
        }

        let key = sol.nifi_key.unwrap();
        let object = match sol.value.0 {
            Term::Literal(v) => v.destruct().0,
            _ => panic!("Not a literal"),
        };

        proc.component
            .comp
            .config
            .properties
            .insert(key.0, Some(object));
    }

    println!("Updating processor");
    client
        .update_processor(&proc.id, &proc)
        .await
        .expect("Nifi failed med");

    (proc.id, proc.component)
}

async fn add_nifi_link<'a>(
    client: &'a Nifi,
    link: NifiLinkQueryOutput<QueryString<"key">>,
    procs: &'a HashMap<String, Component<ProcessorDTO>>,
) -> Option<()> {
    println!("Adding link between processors");

    let source = procs.get(&link.source_id as &str)?;
    let target = procs.get(&link.target_id as &str)?;

    let body = ConnectionEntity::new(&source, &target, Some(&link.key));

    client
        .create_conection(body)
        .await
        .expect("Creating nifi connection");

    Some(())
}
