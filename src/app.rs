use clap::Parser;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Output {
    Turtle,
    Json,
}

impl Default for Output {
    fn default() -> Self {
        Output::Turtle
    }
}

#[derive(clap::Args, Debug)]
pub struct RunArgs {
    #[arg(value_enum, short, long, default_value_t = Output::default())]
    pub output: Output,
    #[arg(value_enum, short, long)]
    pub base: Option<String>,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, color = clap::ColorChoice::Always)]
pub struct Args {
    #[command(flatten)]
    pub run: RunArgs,
    #[command(subcommand)]
    pub action: Action,
}

#[derive(clap::Subcommand, Debug)]
pub enum Action {
    Run {
        #[arg(short, long, default_value_t = String::from("./test.ttl"))]
        ontology: String,
        input: Option<String>,
    },
    /// Get information about the current nifi instance
    Info,
    /// List things
    List {
        #[command(subcommand)]
        action: ListAction,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum ListAction {
    /// List all currently supported nifi processors
    Types {
        #[arg(short, long)]
        filter: Option<Vec<String>>,
    },
    /// List all currently supported nifi controller services
    Services {
        #[arg(short, long)]
        filter: Option<Vec<String>>,
    },
    /// List all properties for a particular nifi processor
    Type {
        #[arg(default_value_t = String::from("be.vlaanderen.informatievlaanderen.ldes.processors.LdesClient"))]
        ty: String,
    },
    Service {
        #[arg(default_value_t = String::from("org.apache.nifi.websocket.jetty.JettyWebSocketClient"))]
        ty: String,
    },
}

pub trait Format {
    fn format(&self, output: Output) -> String;
}
