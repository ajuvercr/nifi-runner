use std::{collections::HashSet, io::Write};

use self::prefix::Prefix;

pub mod prefix {
    pub type Prefix = (&'static str, &'static str);
    pub static SH: Prefix = ("sh", "http://www.w3.org/ns/shacl#");
    pub static XSD: Prefix = ("xsd", "http://www.w3.org/2001/XMLSchema#");
    pub static DCTERMS: Prefix = ("dcterms", "http://purl.org/dc/terms/");
    pub static RDFS: Prefix = ("rdfs", "http://www.w3.org/2000/01/rdf-schema#");
    pub static SDS: Prefix = ("sds", "https://w3id.org/sds#");
    pub static CONN: Prefix = ("", "https://w3id.org/conn#");
    pub static NIFI: Prefix = ("nifi", "https://w3id.org/conn/nifi#");
}

#[derive(Default, Debug)]
pub struct RdfContext {
    prefixes: HashSet<Prefix>,
}

fn prefix_to_string(prefix: &Prefix) -> String {
    format!("@prefix {}: <{}> .\n", prefix.0, prefix.1)
}

impl RdfContext {
    pub fn add_prefix(&mut self, prefix: &Prefix) {
        self.prefixes.insert(prefix.clone());
    }

    pub fn prefixes(&self) -> String {
        self.prefixes.iter().map(prefix_to_string).collect()
    }
}

pub trait ToRDF {
    fn add_ctx(ctx: &mut RdfContext);
    fn to_rdf(self, target: &mut impl Write) -> std::io::Result<()>;
}

impl<T, I> ToRDF for &I
where
    Self: IntoIterator<Item = T>,
    T: ToRDF,
{
    fn add_ctx(ctx: &mut RdfContext) {
        T::add_ctx(ctx);
    }
    fn to_rdf(self, target: &mut impl Write) -> std::io::Result<()> {
        self.into_iter().try_for_each(|x| x.to_rdf(target))
    }
}
