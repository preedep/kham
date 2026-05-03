mod api;
mod collocate;
mod corpus;
mod freq;
mod indexer;
mod kwic;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kham-tnc", about = "Professional Thai corpus analysis tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the web server
    Serve {
        /// Path to SQLite corpus database
        #[arg(long, default_value = "corpus.sqlite")]
        corpus: String,
        /// Port to listen on
        #[arg(long, default_value_t = 8080)]
        port: u16,
        /// URL of the NER validator sidecar (e.g. http://127.0.0.1:9999)
        #[arg(long)]
        validator_url: Option<String>,
    },
    /// Index a text file into a corpus
    Index {
        /// Input file (.txt or .csv)
        file: String,
        /// SQLite corpus database path
        #[arg(long, default_value = "corpus.sqlite")]
        corpus: String,
        /// Genre label for this document
        #[arg(long)]
        genre: Option<String>,
        /// Domain label for this document
        #[arg(long)]
        domain: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("kham_tnc=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Serve {
            corpus,
            port,
            validator_url,
        } => {
            api::serve(&corpus, port, validator_url).await?;
        }
        Command::Index {
            file,
            corpus,
            genre,
            domain,
        } => {
            let db = corpus::CorpusDb::open(&corpus)?;
            indexer::index_file(&db, &file, genre.as_deref(), domain.as_deref())?;
        }
    }

    Ok(())
}
