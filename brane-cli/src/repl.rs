use std::borrow::Cow::{self, Borrowed, Owned};
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use brane_bvm::vm::{Vm, VmOptions};
use brane_drv::grpc::{CreateSessionRequest, DriverServiceClient, ExecuteRequest};
use brane_dsl::{Compiler, CompilerOptions, Lang};
use log::warn;
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::config::OutputStreamType;
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::{self, MatchingBracketValidator, Validator};
use rustyline::{CompletionType, Config, Context, EditMode, Editor};
use rustyline_derive::Helper;

use crate::docker::DockerExecutor;
use crate::errors::ReplError;
use crate::packages;
use crate::utils::{ensure_config_dir, get_history_file};


/***** REPL HELPER *****/
/// Implements the helper for the Repl (auto-completion and syntax highlighting and such)
#[derive(Helper)]
struct ReplHelper {
    /// The completer: we auto-complete filenames, like the standard terminal
    completer      : FilenameCompleter,
    /// Highlighter: we highlight matching brackets
    highlighter    : MatchingBracketHighlighter,
    /// We even validate for matching brackets
    validator      : MatchingBracketValidator,
    /// We hint based on the user's history
    hinter         : HistoryHinter,
    /// Does something with being a coloured prompt(?)
    colored_prompt : String,
}

impl Completer for ReplHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        self.completer.complete(line, pos, ctx)
    }
}

impl Hinter for ReplHelper {
    type Hint = String;

    fn hint(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> Option<String> {
        self.hinter
            .hint(line, pos, ctx)
            .and_then(|h| h.lines().next().map(|l| l.to_string()))
    }
}

impl Highlighter for ReplHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Borrowed(&self.colored_prompt)
        } else {
            Borrowed(prompt)
        }
    }

    fn highlight_hint<'h>(
        &self,
        hint: &'h str,
    ) -> Cow<'h, str> {
        Owned("\x1b[1m".to_owned() + hint + "\x1b[m")
    }

    fn highlight<'l>(
        &self,
        line: &'l str,
        pos: usize,
    ) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(
        &self,
        line: &str,
        pos: usize,
    ) -> bool {
        self.highlighter.highlight_char(line, pos)
    }
}

impl Validator for ReplHelper {
    fn validate(
        &self,
        ctx: &mut validate::ValidationContext,
    ) -> rustyline::Result<validate::ValidationResult> {
        self.validator.validate(ctx)
    }

    fn validate_while_typing(&self) -> bool {
        self.validator.validate_while_typing()
    }
}





/***** SUBCOMMANDS *****/
/// Entrypoint to the REPL, which performs the required initialization.
/// 
/// **Arguments**
///  * `bakery`: Whether to use BraneScript (false) or Bakery (true).
///  * `clear`: Whether or not to clear the history of the REPL before beginning.
///  * `remote`: Whether or not to connect to a remote Brane Instance (address is given if Some).
///  * `attach`: If not None, defines the session ID of an existing session to connect to.
///  * `data`: Whether or not to mount a particular folder for the data directory.
/// 
/// **Returns**  
/// Nothing on success, or else a ReplError.
pub async fn start(
    bakery: bool,
    clear: bool,
    remote: Option<String>,
    attach: Option<String>,
    data: Option<PathBuf>,
) -> Result<(), ReplError> {
    // Build the config for the rustyline REPL.
    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::Circular)
        .edit_mode(EditMode::Emacs)
        .output_stream(OutputStreamType::Stdout)
        .build();

    // Build the helper for the REPL
    let repl_helper = ReplHelper {
        completer: FilenameCompleter::new(),
        highlighter: MatchingBracketHighlighter::new(),
        hinter: HistoryHinter {},
        colored_prompt: "".to_owned(),
        validator: MatchingBracketValidator::new(),
    };

    // Get the history file, clearing it if necessary
    if let Err(err) = ensure_config_dir(true) { return Err(ReplError::ConfigDirCreateError{ err }); };
    let history_file = match get_history_file() {
        Ok(file) => file,
        Err(err) => { return Err(ReplError::HistoryFileError{ err }); }
    };
    if clear && history_file.exists() {
        if let Err(err) = fs::remove_file(&history_file) {
            warn!("Could not clear REPL history: {}", err);
        };
    }

    // Create the REPL
    let mut rl = Editor::with_config(config);
    rl.set_helper(Some(repl_helper));
    if let Err(err) = rl.load_history(&history_file) { warn!("Could not load REPL history from '{}': {}", history_file.display(), err); }

    // Initialization done; run the REPL
    println!("Welcome to the Brane REPL, press Ctrl+D to exit.\n");
    if let Some(remote) = remote {
        remote_repl(&mut rl, bakery, remote, attach).await?;
    } else {
        local_repl(&mut rl, bakery, data).await?;
    }

    // Try to save the history if we exited cleanly
    if let Err(reason) = rl.save_history(&history_file) {
        warn!("Could not save session history to '{}': {}", history_file.display(), reason);
    }

    // Done!
    Ok(())
}



/// Implements a REPL that connects to a remote host.
/// 
/// **Arguments**
///  * `rl`: The RustyLine editor that we use to get user input.
///  * `bakery`: Whether to use BraneScript (false) or Bakery (true).
///  * `remote`: The remote address to connect to.
///  * `attach`: If not None, defines the session ID of an existing session to connect to.
/// 
/// **Returns**  
/// Nothing on success, or else a ReplError.
async fn remote_repl(
    rl: &mut Editor<ReplHelper>,
    _bakery: bool,
    remote: String,
    attach: Option<String>,
) -> Result<(), ReplError> {
    // Connect to the server with gRPC
    let mut client = match DriverServiceClient::connect(remote.clone()).await {
        Ok(client) => client,
        Err(err)   => { return Err(ReplError::ClientConnectError{ address: remote, err }); }
    };

    // Either use the given Session UUID or create a new one (with matching session)
    let session = if let Some(attach) = attach {
        attach.clone()
    } else {
        // Setup a new session
        let request = CreateSessionRequest {};
        let reply = match client.create_session(request).await {
            Ok(reply) => reply,
            Err(err)  => { return Err(ReplError::SessionCreateError{ address: remote, err }); }
        };

        // Return the UUID of this session
        reply.into_inner().uuid.clone()
    };

    // With the status setup, enter the L in the REPL
    let mut count: u32 = 1;
    loop {
        // Prepare the prompt with the current iteration number
        let p = format!("{}> ", count);

        // Write the prompt in a coloured way
        rl.helper_mut().expect("No helper").colored_prompt = format!("\x1b[1;32m{}\x1b[0m", p);

        // Wait until the user provided us with some command
        let readline = rl.readline(&p);
        match readline {
            Ok(line) => {
                // The command checked out, so add it to the history
                rl.add_history_entry(line.as_str());

                // Prepare the request to execute this command
                let request = ExecuteRequest {
                    uuid: session.clone(),
                    input: line.clone(),
                };

                // Run it
                let response = match client.execute(request).await {
                    Ok(response) => response,
                    Err(err)     => { return Err(ReplError::CommandRequestError{ address: remote, err }); }
                };
                let mut stream = response.into_inner();

                // Switch on the type of message that the remote returned
                #[allow(irrefutable_let_patterns)]
                while let message = stream.message().await {
                    match message {
                        // The message itself went alright
                        Ok(Some(reply)) => {
                            // The remote send us some debug message
                            if let Some(debug) = reply.debug {
                                debug!("Remote: {}", debug);
                            }

                            // The remote send us a normal text message
                            if let Some(stdout) = reply.stdout {
                                debug!("Remote returned stdout");
                                println!("{}", stdout);
                            }

                            // The remote send us an error
                            if let Some(stderr) = reply.stderr {
                                debug!("Remote returned error");
                                eprintln!("{}", stderr);
                            }

                            // The remote is done with this
                            if reply.close {
                                break;
                            }
                        }
                        Err(status) => {
                            // Did not receive the message properly
                            eprintln!("\nStatus error: {}", status.message());
                            break;
                        }
                        Ok(None) => {
                            // Stream closed(?)
                            break;
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("Keyboard interrupt not supported. Press Ctrl+D to exit.");
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                // Something went wrong getting user input
                println!("Error: {:?}", err);
                break;
            }
        }

        // Increment the count and try the next iteration of the REPL
        count += 1;
    }

    // Exit cleanly
    Ok(())
}



/// Implements a REPL that runs stuff on the local Docker daemon.
/// 
/// *Arguments**
///  * `rl`: The RustyLine editor that we use to get user input.
///  * `bakery`: Whether to use BraneScript (false) or Bakery (true).
///  * `data`: Whether or not to mount a particular folder for the data directory.
/// 
/// **Returns**  
/// Nothing on success, or else a ReplError.
async fn local_repl(
    rl: &mut Editor<ReplHelper>,
    bakery: bool,
    data: Option<PathBuf>,
) -> Result<(), ReplError> {
    // Setup the compiler options for the appropriate language
    let compiler_options = if bakery {
        CompilerOptions::new(Lang::Bakery)
    } else {
        CompilerOptions::new(Lang::BraneScript)
    };

    // Get the package index for the local repository
    let package_index = match packages::get_package_index() {
        Ok(index) => index,
        Err(err)  => { return Err(ReplError::PackageIndexError{ err }); }
    };

    // Create the compiler for the appropriate language and knowing of the local packages
    let mut compiler = Compiler::new(compiler_options, package_index.clone());

    // Initialize the local executor
    let executor = DockerExecutor::new(data);
    let options = VmOptions {
        clear_after_main: true,
        ..Default::default()
    };
    let mut vm = match Vm::new_with(executor, Some(package_index), Some(options)) {
        Ok(vm)   => vm,
        Err(err) => { return Err(ReplError::VmCreateError{ err }); }
    };

    // With the VM setup, enter the L in the REPL
    let mut count: u32 = 1;
    loop {
        // Prepare the prompt with the current iteration number
        let p = format!("{}> ", count);

        // Write the prompt in a coloured way
        rl.helper_mut().expect("No helper").colored_prompt = format!("\x1b[1;32m{}\x1b[0m", p);

        // Wait until the user provided us with some command
        let readline = rl.readline(&p);
        match readline {
            Ok(line) => {
                // The command checked out, so add it to the history
                rl.add_history_entry(line.as_str());

                // Compile it
                match compiler.compile(line) {
                    Ok(function) => {
                        // Call the virtual machine to execute the instructions
                        if let Err(reason) = vm.main(function).await {
                            // Do not throw an error, but simply write what went wrong and allow the user to try again
                            eprintln!("{}", reason);
                        }
                    },
                    Err(error) => eprintln!("{:?}", error),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("Keyboard interrupt not supported. Press Ctrl+D to exit.");
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                // Something went wrong getting user input
                println!("Error: {:?}", err);
                break;
            }
        }

        // Increment the count and try the next iteration of the REPL
        count += 1;
    }

    // Exit cleanly
    Ok(())
}
