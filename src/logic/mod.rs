use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;

use crate::client::Nifi;
use crate::models::{Component, ProcessorDTO};
use crate::util::*;
use oxigraph::store::Store;
use rio_api::parser::TriplesParser;
use rio_turtle::{TurtleError, TurtleParser};

mod logic;
mod reader;
mod writer;
pub use logic::*;

async fn template_file_id(client: &Nifi, location: &str) -> Option<String> {
    let content = std::fs::read_to_string(location).ok()?;
    client.upload_template(content).await.ok()
}

pub fn import_reader_to_store<R: BufRead + Sized>(file: R, bl: &Store) -> std::io::Result<()> {
    let parser = TurtleParser::new(file, None);
    let mut mapper = RDFMapper::default();
    parser
        .into_iter::<_, TurtleError, _>(|triple| Ok(mapper.map_triple_to_quad(triple)))
        .flatten()
        .for_each(|q| {
            bl.insert(&q).unwrap();
        });

    Ok(())
}

pub fn import_file_to_store<P: AsRef<Path>>(location: P, bl: &Store) -> std::io::Result<()> {
    import_reader_to_store(BufReader::new(File::open(location)?), &bl)
}

#[async_trait::async_trait]
pub trait Channel {
    fn append_ontology(store: &Store) -> std::io::Result<()>;

    // async fn add_channel_reader(
    //     client: Nifi,
    //     store: &Store,
    //     procs: &HashMap<String, Component<ProcessorDTO>>,
    // );
    //
    async fn add_channel_writer(
        client: Arc<Nifi>,
        store: Arc<Store>,
        procs: Arc<HashMap<String, Component<ProcessorDTO>>>,
    );
}
