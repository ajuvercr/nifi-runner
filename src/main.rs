#![feature(adt_const_params)]
use crate::models::DocumentedTypeDTO;
use app::{Actives, ListAction, Output, RunArgs};
use clap::Parser;
use client::Nifi;
use oxiri::Iri;
use rdf::{RdfContext, ToRDF};
use rio_turtle::{TurtleFormatter, TurtleParser};
use serde::Serialize;
use std::io::Write;
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
mod sparql;
mod util;

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

            cursor.write_all(prefixes.as_bytes()).unwrap();
            item.to_rdf(&mut cursor).unwrap();
            cursor.set_position(0);

            std::io::copy(&mut cursor, &mut stdout().lock()).unwrap();

            // use rio_api::formatter::TriplesFormatter;
            // use rio_api::parser::TriplesParser;
            // let mut turtle_fmt = TurtleFormatter::new(stdout());
            //
            // let mut turtle_parser =
            //     TurtleParser::new(cursor, Some(Iri::parse(base.to_string()).unwrap()));
            // turtle_parser
            //     .parse_all(&mut |x| turtle_fmt.format(&x))
            //     .unwrap();
            //
            // turtle_fmt.finish().unwrap().flush().unwrap();
        }
    }
}

fn filter_list(
    items: Vec<DocumentedTypeDTO>,
    filter: Option<Vec<String>>,
) -> Vec<DocumentedTypeDTO> {
    let types = if let Some(filters) = filter {
        let filters_lower_case: Vec<_> = filters.into_iter().map(|x| x.to_lowercase()).collect();

        items
            .into_iter()
            .filter(|x| {
                x.description
                    .as_ref()
                    .map(|x| x.to_lowercase())
                    .map(|x| filters_lower_case.iter().any(|y| x.contains(y)))
                    .unwrap_or_default()
                    || x.tags
                        .iter()
                        .map(|x| x.to_lowercase())
                        .any(|x| filters_lower_case.iter().any(|y| x.contains(y)))
            })
            .collect()
    } else {
        items
    };

    types
}

async fn handle_list_action(
    client: Nifi,
    action: ListAction,
    output: RunArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ListAction::Types { filter, full } => {
            let types = client.list_types().await?;
            let types = filter_list(types.types, filter);
            if full {
                let mut out = Vec::new();
                for p in types {
                    let processor = client.new_processor(&p.ty).await?;
                    client.delete_processor(&processor.id, 1).await?;
                    out.push(processor);
                }
                format_output(&out, output);
            } else {
                format_output(&types, output);
            }
        }
        ListAction::Services { filter } => {
            let types = client.list_services_types().await?;
            let types = filter_list(types.types, filter);
            format_output(&types, output);
        }
        ListAction::Type { ty } => {
            let processor = client.new_processor(&ty).await?;

            format_output(&processor, output);

            client.delete_processor(&processor.id, 1).await?;
        }
        ListAction::Service { ty } => {
            let processor = client.new_service(&ty).await?;

            format_output(&processor, output);

            client.delete_service(&processor.id, 1).await?;
        }
        ListAction::Active {
            ty: Actives::Service,
        } => {
            let active = client.list_services(true).await?;
            println!("{}", serde_json::to_string_pretty(&active).unwrap());
        }
        ListAction::Active {
            ty: Actives::Processor,
        } => {
            let active = client.list_active_processors().await?;
            println!("{}", serde_json::to_string_pretty(&active).unwrap());
        }
        ListAction::Active { ty: Actives::Group } => {
            let active = client.get_process_group().await?;
            println!("{}", serde_json::to_string_pretty(&active).unwrap());
        }
    }
    Ok(())
}

// No multithreading required
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.action {
        app::Action::Run {
            ontology,
            input,
            no_start,
        } => {
            logic::startup(args.client, ontology, input, !no_start).await;
        }
        app::Action::Info => {
            print_result(args.client.get_info().await)?;
        }
        app::Action::List { action } => {
            handle_list_action(args.client, action, args.run).await?;
        }
        app::Action::Testing => handle_testing(args.client, args.run).await?,
    }

    Ok(())
}

async fn handle_testing(client: Nifi, run: RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    client.start_process_group().await?;
    Ok(())
}
