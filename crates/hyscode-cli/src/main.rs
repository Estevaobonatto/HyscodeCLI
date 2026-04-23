//! HyscodeCLI — Agente de codificação em linha de comando
//!
//! Entry point do binário. Parse de argumentos e dispatch para comandos.

use clap::{CommandFactory, Parser, Subcommand};

mod commands;
mod oauth;

#[derive(Parser)]
#[command(
    name = "hyscode",
    about = "HyscodeCLI — Agente de codificação com IA",
    long_about = None,
    version,
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Provedor de LLM a usar (sobrescreve configuração)
    #[arg(long, short = 'p', global = true)]
    provider: Option<String>,

    /// Modelo a usar (sobrescreve configuração)
    #[arg(long, short = 'm', global = true)]
    model: Option<String>,

    /// Nível de verbosidade (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Inicia uma conversa interativa com o modelo
    Chat {
        /// Mensagem inicial (se omitida, abre modo interativo)
        message: Option<String>,

        /// Arquivo(s) ou diretório(s) como contexto adicional
        #[arg(long, short = 'c')]
        context: Vec<String>,
    },

    /// Executa o agente de forma autônoma para completar uma tarefa
    Agent {
        /// Descrição da tarefa a ser executada
        #[arg(long, short = 't')]
        task: String,

        /// Aprovar automaticamente operações destrutivas
        #[arg(long)]
        auto_approve: bool,

        /// Apenas mostrar ações, não executar
        #[arg(long)]
        audit_only: bool,
    },

    /// Gerencia provedores de LLM
    Provider {
        #[command(subcommand)]
        action: ProviderAction,
    },

    /// Gerencia configurações
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Inicializa o HyscodeCLI no diretório atual
    Init,

    /// Lista o histórico de conversas
    History {
        /// Número de conversas a exibir
        #[arg(long, short = 'n', default_value = "20")]
        limit: usize,
    },

    /// Desfaz a(s) última(s) escrita(s) de arquivo pelo agente
    Undo {
        /// Quantas operações desfazer
        #[arg(default_value = "1")]
        steps: usize,
    },

    /// Revisa o diff atual com o LLM
    Review {
        /// Revisar apenas mudanças staged (padrão: todas as mudanças)
        #[arg(long, short = 's')]
        staged: bool,
    },

    /// Gera mensagem de commit para o repositório atual
    Commit {
        /// Stage automaticamente os arquivos modificados
        #[arg(long, short = 'a')]
        all: bool,
    },

    /// Gera script de autocompleção para o shell especificado
    Completions {
        /// Shell alvo (bash, zsh, fish, powershell, elvish)
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
enum ProviderAction {
    /// Adiciona e configura um provedor
    Add {
        name: String,
        #[arg(long)]
        api_key: Option<String>,
    },
    /// Lista provedores configurados
    List,
    /// Remove um provedor
    Remove { name: String },
    /// Define o provedor padrão
    Default { name: String },
    /// Testa conectividade e credenciais
    Test { name: String },
    /// Autentica via OAuth (GitHub Copilot)
    Login { name: String },
    /// Lista modelos disponíveis de um provedor
    Models { name: String },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Exibe uma configuração
    Get { key: String },
    /// Define uma configuração
    Set { key: String, value: String },
    /// Exibe o arquivo de configuração
    Show,
    /// Abre o arquivo de configuração no editor
    Edit,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Inicializa logging com base no nível de verbosidade
    init_logging(cli.verbose);

    // Despacha para o comando correto
    match cli.command {
        Commands::Chat { message, context } => {
            commands::chat::run(message, context, cli.provider, cli.model).await
        }
        Commands::Agent { task, auto_approve, audit_only } => {
            commands::agent::run(task, auto_approve, audit_only, cli.provider, cli.model).await
        }
        Commands::Provider { action } => {
            commands::provider::run(action).await
        }
        Commands::Config { action } => {
            commands::config::run(action).await
        }
        Commands::Init => commands::init::run().await,
        Commands::History { limit } => commands::history::run(limit).await,
        Commands::Undo { steps } => commands::undo::run(steps).await,
        Commands::Review { staged } => {
            commands::review::run(staged, cli.provider, cli.model).await
        }
        Commands::Commit { all } => commands::commit::run(all).await,
        Commands::Completions { shell } => {
            commands::completions::run(shell);
            Ok(())
        }
    }
}

fn init_logging(verbose: u8) {
    use tracing_subscriber::{fmt, EnvFilter};

    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let filter = EnvFilter::try_from_env("HYSCODE_LOG")
        .unwrap_or_else(|_| EnvFilter::new(level));

    fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_target(verbose >= 2)
        .with_thread_ids(false)
        .compact()
        .init();
}
