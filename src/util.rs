use std::collections::HashMap;

use oxigraph::model::{BlankNode, GraphName, Literal, NamedNode, Quad, Subject, Term, Triple};
use rio_api::model as rm;

#[derive(Default)]
pub struct RDFMapper {
    blanks: HashMap<String, BlankNode>,
}

impl RDFMapper {
    fn blank_node(&mut self, id: &str) -> BlankNode {
        if let Some(bn) = self.blanks.get(id) {
            bn.clone()
        } else {
            let out = BlankNode::default();
            self.blanks.insert(id.to_string(), out.clone());
            out
        }
    }

    // #[inline]
    pub fn map_triple<'a>(&mut self, sub: &'a rm::Triple<'a>) -> Triple {
        Triple::new(
            self.map_subject(&sub.subject),
            self.map_predicate(&sub.predicate),
            self.map_object(&sub.object),
        )
    }

    // #[inline]
    pub fn map_subject<'a>(&mut self, sub: &'a rm::Subject<'a>) -> Subject {
        match sub {
            rm::Subject::NamedNode(n) => Subject::NamedNode(NamedNode::new_unchecked(n.iri)),
            rm::Subject::BlankNode(n) => Subject::BlankNode(self.blank_node(n.id)),
            rm::Subject::Triple(n) => Subject::Triple(Box::new(self.map_triple(n))),
        }
    }

    // #[inline]
    pub fn map_predicate<'a>(&mut self, pred: &'a rm::NamedNode<'a>) -> NamedNode {
        NamedNode::new_unchecked(pred.iri)
    }

    // #[inline]
    pub fn map_literal<'a>(&mut self, lit: &'a rm::Literal<'a>) -> Literal {
        match lit {
            rm::Literal::Simple { value } => Literal::new_simple_literal(*value),
            rm::Literal::LanguageTaggedString { value, language } => {
                Literal::new_language_tagged_literal_unchecked(*value, *language)
            }
            rm::Literal::Typed { value, datatype } => {
                Literal::new_typed_literal(*value, self.map_predicate(datatype))
            }
        }
    }

    // #[inline]
    pub fn map_object<'a>(&mut self, object: &'a rm::Term<'a>) -> Term {
        match object {
            rm::Term::NamedNode(n) => Term::NamedNode(NamedNode::new_unchecked(n.iri)),
            rm::Term::BlankNode(n) => Term::BlankNode(self.blank_node(n.id)),
            rm::Term::Literal(n) => Term::Literal(self.map_literal(n)),
            rm::Term::Triple(n) => Term::Triple(Box::new(self.map_triple(n))),
        }
    }

    // #[inline]
    pub fn map_triple_to_quad(
        &mut self,
        rm::Triple {
            subject,
            predicate,
            object,
        }: rm::Triple,
    ) -> Quad {
        Quad::new(
            self.map_subject(&subject),
            self.map_predicate(&predicate),
            self.map_object(&object),
            GraphName::DefaultGraph,
        )
    }
}

pub fn unwrap_literal<'a>(x: &'a Term) -> Option<&str> {
    match x {
        Term::Literal(l) => Some(l.value()),
        _ => None,
    }
}
