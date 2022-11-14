use app::{ListAction, Output, RunArgs};
use clap::Parser;
use models::DocumentedTypeDTO;
use rdf::{RdfContext, ToRDF};
use rio_turtle::{TurtleFormatter, TurtleParser};
use serde::Serialize;
use std::{
    fmt::Debug,
    io::{stdout, Cursor},
};

use crate::app::Args;
pub mod app;
pub mod client;
pub mod logic;
pub mod models;
pub mod rdf;

const BASE_URI: &str = "http://example.com/ns#";

fn print_result<T: Serialize, E: Debug>(r: Result<T, E>) -> Result<T, E> {
    match r {
        Ok(ref x) => println!("{}", serde_json::to_string_pretty(&x).unwrap()),
        Err(ref e) => println!("{:?}", e),
    }

    r
}

fn format_output<T: Serialize + ToRDF>(item: T, args: RunArgs) {
    let base = args.base.as_ref().map(|x| x.as_str()).unwrap_or(BASE_URI);
    match args.output {
        Output::Json => {
            let out = serde_json::to_string_pretty(&item)
                .unwrap_or_else(|_| String::from("Failed to serialize"));
            println!("{}", out);
        }
        Output::Turtle => {
            let mut cursor = Cursor::new(Vec::new());
            let mut ctx = RdfContext::default();
            <T>::add_ctx(&mut ctx);
            let prefixes = ctx.prefixes();
            write!(cursor, "@base <{}> .\n", base).unwrap();
            cursor.write_all(prefixes.as_bytes()).unwrap();
            item.to_rdf(&mut cursor).unwrap();
            cursor.set_position(0);

            let mut turtle_fmt = TurtleFormatter::new(stdout());

            let mut turtle_parser = TurtleParser::new(cursor, None);
            turtle_parser
                .parse_all(&mut |x| turtle_fmt.format(&x))
                .unwrap();

            turtle_fmt.finish().unwrap().flush().unwrap();
        }
    }
}

fn list_and_print(items: Vec<DocumentedTypeDTO>, filter: Option<Vec<String>>, args: RunArgs) {
    let types = if let Some(filters) = filter {
        let filters_lower_case: Vec<_> = filters.into_iter().map(|x| x.to_lowercase()).collect();

        items
            .into_iter()
            .filter(|x| {
                x.tags
                    .iter()
                    .any(|y| filters_lower_case.contains(&y.to_lowercase()))
            })
            .collect()
    } else {
        items
    };

    format_output(&types, args);
}

use rio_api::formatter::TriplesFormatter;
use rio_api::parser::TriplesParser;
use std::io::Write;
async fn handle_list_action(
    action: ListAction,
    output: RunArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ListAction::Types { filter } => {
            let types = client::Nifi::list_types().await?;
            list_and_print(types.types, filter, output);
        }
        ListAction::Services { filter } => {
            let types = client::Nifi::list_services().await?;
            list_and_print(types.types, filter, output);
        }
        ListAction::Type { ty } => {
            let processor = client::Nifi::new_processor("root", &ty).await?;

            format_output(&processor, output);

            client::Nifi::delete_processor(&processor.id, 1).await?;
        }
        ListAction::Service { ty } => {
            let processor = client::Nifi::new_service("root", &ty).await?;

            format_output(&processor, output);

            client::Nifi::delete_service(&processor.id, 1).await?;
        }
    }
    Ok(())
}

// No multithreading required
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.action {
        app::Action::Run { ontology, input } => {
            logic::startup(ontology, input).await;
        }
        app::Action::Info => {
            print_result(client::Nifi::get_info().await)?;
        }
        app::Action::List { action } => {
            handle_list_action(action, args.run).await?;
        }
    }

    Ok(())
}
