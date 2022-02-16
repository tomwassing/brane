use anyhow::Result;
use socksx::socks6::options::SocksOption;
use socksx::{self, Socks6Client};
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FResult};
use std::net::IpAddr;
use std::process::Command;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};


/***** CONSTANTS *****/
/// The standard address where the Redirector is bound to
const REDIRECTOR_ADDRESS: &str = "127.0.0.1:42000";





/***** ERRORS *****/
/// Collects errors for the Redirection service.
#[derive(Debug)]
pub enum RedirectorError {
    /// Socksx could not resolve the proxy address
    AddressResolveError{ address: String, err: anyhow::Error },

    /// Could not run the command to alter the iptables
    IptablesLaunchError{ command: String, err: std::io::Error },
    /// The iptables command failed somehow
    IptablesError{ command: String, code: i32, stdout: String, stderr: String },

    /// Could not bind a TCP server to the local address
    ServerBindError{ address: String, err: std::io::Error },
    /// Could not bind a TCP client to the remote proxy
    ClientBindError{ address: String, err: anyhow::Error },

    /// Could not accept an incoming connection
    ServerAcceptError{ err: std::io::Error },
    /// Could not get the original destination for the input stream.
    OriginalDestinationError{ err: anyhow::Error },
    /// Could not connect client to the given host
    ClientConnectError{ address: String, err: anyhow::Error },
    /// Failed to actually to the traffic redirection
    ClientRedirectError{ address: String, err: std::io::Error },
}

impl Display for RedirectorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        match self {
            RedirectorError::AddressResolveError{ address, err } => write!(f, "Could not resolve address '{}': {}", address, err),

            RedirectorError::IptablesLaunchError{ command, err }            => write!(f, "Could not run command '{}': {}", command, err),
            RedirectorError::IptablesError{ command, code, stdout, stderr } => write!(f, "Iptables update command '{}' returned exit code {}:\n\nstdout:\n{}\n{}\n{}\n\nstderr:\n{}\n{}\n{}\n\n", command, code, (0..80).map(|_| '-').collect::<String>(), stdout, (0..80).map(|_| '-').collect::<String>(), (0..80).map(|_| '-').collect::<String>(), stderr,(0..80).map(|_| '-').collect::<String>()),

            RedirectorError::ServerBindError{ address, err } => write!(f, "Could not bind TCP listener to address '{}': {}", address, err),
            RedirectorError::ClientBindError{ address, err } => write!(f, "Could not bind TCP client to proxy with address '{}': {}", address, err),

            RedirectorError::ServerAcceptError{ err }            => write!(f, "Could not accept incoming connection: {}", err),
            RedirectorError::OriginalDestinationError{ err }     => write!(f, "Could not get original address from incoming TCP stream: {}", err),
            RedirectorError::ClientConnectError{ address, err }  => write!(f, "Could not connect client to '{}': {}", address, err),
            RedirectorError::ClientRedirectError{ address, err } => write!(f, "Could not copy redirected traffic to '{}': {}", address, err),
        }
    }
}

impl Error for RedirectorError {}





/***** LIBRARY FUNCTIONS *****/
/// **Edited: now returning RedirectorErrors.**
/// 
/// Starts the background Redirector service on Tokio and in the iptables.
/// 
/// **Arguments**
///  * `proxy_address`: The address to redirect all traffic to.
///  * `options`: Possible options for the socksx library used.
/// 
/// **Returns**  
/// Nothing if the service started successfully, or a RedirectorError on failure.
pub async fn start(
    proxy_address: String,
    options: Vec<SocksOption>,
) -> Result<(), RedirectorError> {
    // Try to resolve the socket address
    let proxy_ip = match socksx::resolve_addr(&proxy_address).await {
        Ok(proxy_ip) => proxy_ip.ip(),
        Err(err)     => { return Err(RedirectorError::AddressResolveError{ address: proxy_address, err }); }
    };
    debug!("Going to setup network redirection to proxy with IP: {}.", proxy_ip);

    // Turn interception on as quickly as possible.
    configure_iptables(&proxy_ip)?;

    // Create a TCP listener that will receive intercepted connections.
    let listener: TcpListener = match TcpListener::bind(REDIRECTOR_ADDRESS).await {
        Ok(listener) => listener,
        Err(err)     => { return Err(RedirectorError::ServerBindError{ address: REDIRECTOR_ADDRESS.to_string(), err }); }
    };
    // Create a client to the proxy address
    let client = match Socks6Client::new(proxy_address.clone(), None).await {
        Ok(client) => client,
        Err(err)   => { return Err(RedirectorError::ClientBindError{ address: proxy_address, err }); }
    };

    // Spawn the actual redirector service
    tokio::spawn(async move {
        debug!("Started redirector service on: {}", REDIRECTOR_ADDRESS);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    tokio::spawn(redirect(stream, client.clone(), options.clone()));
                }
                Err(err) => {
                    error!("{}", RedirectorError::ServerAcceptError{ err });
                    break;
                }
            }
        }
    });

    // Everything *should* be instantaneous, but give it some time to be sure.
    tokio::time::sleep(Duration::from_millis(256)).await;
    Ok(())
}

/// **Edited: now returning RedirectorErrors.**
/// 
/// Configures the container's iptables to redirect all network to the Redirector service.
/// 
/// **Arguments**
///  * `proxy_ip`: The IP-address of the proxy we want to redirect to.
/// 
/// **Returns**  
/// Returns nothing if the iptables were configured successfully, or a RedirectorError otherwise.
fn configure_iptables(proxy_ip: &IpAddr) -> Result<(), RedirectorError> {
    // Get the string counterpart of the IP
    let proxy_ip = proxy_ip.to_string();

    // Define the arguments used for the iptables
    let args = format!(
        "-t nat -A OUTPUT ! -d {}/32 -o eth0 -p tcp -m tcp -j REDIRECT --to-ports 42000",
        proxy_ip
    );
    let args: Vec<&str> = args.split_ascii_whitespace().collect();

    // Check if we could run
    let mut command = Command::new("iptables");
    command.args(&args);
    let output = match command.output() {
        Ok(output) => output,
        Err(err)   => { return Err(RedirectorError::IptablesLaunchError{ command: format!("{:?}", command), err }); }
    };

    // Stop execution if we can't properly configure IPTables.
    if !output.status.success() {
        return Err(RedirectorError::IptablesError{ command: format!("{:?}", command), code: output.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&output.stdout).to_string(), stderr: String::from_utf8_lossy(&output.stderr).to_string() });
    }

    debug!("Configured IPTables to intercept all outgoing TCP connections.");

    Ok(())
}

/// **Edited: now returning RedirectorErrors.**
/// 
/// Performs a redirection via the proxy server.  
/// Any errors will be logged to stderr.
/// 
/// **Arguments**
///  * `incoming`: The incoming stream to redirect.
///  * `client`: The client to which to write to the proxy server.
///  * `options`: Possible options to launch a new client.
async fn redirect(
    incoming: TcpStream,
    client: Socks6Client,
    options: Vec<SocksOption>,
) {
    let mut incoming = incoming;
    let dst_addr = match socksx::get_original_dst(&incoming) {
        Ok(dst_addr) => dst_addr,
        Err(err)     => { error!("{}", RedirectorError::OriginalDestinationError{ err }); return; }
    };

    debug!("Intercepted connection to: {:?}.", dst_addr);

    let mut outgoing = match client.connect(dst_addr, None, Some(options)).await {
        Ok((outgoing, _)) => outgoing,
        Err(err)          => { error!("{}", RedirectorError::ClientConnectError{ address: dst_addr.to_string(), err }); return; }
    };
    if let Err(err) = tokio::io::copy_bidirectional(&mut incoming, &mut outgoing).await {
        error!("{}", RedirectorError::ClientRedirectError{ address: dst_addr.to_string(), err });
    }
}
