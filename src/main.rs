use crate::models::{ConnectionEntity, DocumentedTypeDTO};
use app::{Actives, CreateAction, ListAction, Output, RunArgs};
use clap::Parser;
use client::Nifi;
use rdf::{RdfContext, ToRDF};
use serde::Serialize;
use std::io::Write;
use std::{
    fmt::Debug,
    io::{stdout, Cursor},
};

use crate::{app::Args, client::PortType};

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
            write!(cursor, "@base <{}> .\n", base).unwrap();
            cursor.write_all(prefixes.as_bytes()).unwrap();
            item.to_rdf(&mut cursor).unwrap();
            cursor.set_position(0);

            std::io::copy(&mut cursor, &mut stdout().lock()).unwrap();

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

async fn handle_list_action(
    client: Nifi,
    action: ListAction,
    output: RunArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ListAction::Types { filter } => {
            let types = client.list_types().await?;
            list_and_print(types.types, filter, output);
        }
        ListAction::Services { filter } => {
            let types = client.list_services().await?;
            list_and_print(types.types, filter, output);
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
            let active = client.list_active_services().await?;
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
        app::Action::Run { ontology, input } => {
            logic::startup(args.client, ontology, input).await;
        }
        app::Action::Info => {
            print_result(args.client.get_info().await)?;
        }
        app::Action::List { action } => {
            handle_list_action(args.client, action, args.run).await?;
        }
        app::Action::Create { action } => {
            handle_create_action(args.client, action, args.run).await?;
        }
    }

    Ok(())
}

async fn handle_create_action(
    client: Nifi,
    action: CreateAction,
    run: RunArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        CreateAction::Group { name } => {
            let target = client
                .new_processor("be.vlaanderen.informatievlaanderen.ldes.processors.LdesClient")
                .await?;
            println!("created target processor");

            let process_group = client.new_process_group(&name).await?;
            println!("created new group");

            let group_client = client.change_group(&process_group.id);

            // let target = group_client
            //     .new_processor("be.vlaanderen.informatievlaanderen.ldes.processors.LdesClient")
            //     .await?;
            // println!("created target processor");

            let output_port = group_client.new_port(PortType::Output, "toRoot").await?;
            println!("created port {:?}", output_port);

            let entity = ConnectionEntity::new(&output_port.component, &target.component, None);

            println!("Entity: {:?}", entity);
            let res = client.create_conection(entity).await;
            println!("Result {}", res.is_ok());
        }
    }
    Ok(())
}
