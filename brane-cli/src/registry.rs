use std::fs::{self, File};
use std::io::prelude::*;
use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::DateTime;
use chrono::Utc;
use console::style;
use console::{pad_str, Alignment};
use dialoguer::Confirm;
use flate2::write::GzEncoder;
use flate2::Compression;
use graphql_client::{GraphQLQuery, Response};
use indicatif::{ProgressBar, ProgressStyle};
use prettytable::format::FormatBuilder;
use prettytable::Table;
use reqwest::{self, Body, Client};
use tokio::fs::File as TokioFile;
use tokio_util::codec::{BytesCodec, FramedRead};
use url::Url;
use uuid::Uuid;

use specifications::package::{PackageKind, PackageInfo};
use specifications::registry::RegistryConfig;
use specifications::version::Version;

use crate::utils::{get_config_dir, get_package_dir, ensure_package_dir, get_package_versions, ensure_packages_dir, ensure_config_dir};


type DateTimeUtc = DateTime<Utc>;


/// Get the GraphQL endpoint of the Brane API.
pub fn get_graphql_endpoint() -> Result<String> {
    // Get the configuration directory
    let config_file = get_config_dir().unwrap().join("registry.yml");
    let config = RegistryConfig::from_path(&config_file)
        .with_context(|| "No registry configuration found, please use `brane login` first.")?;

    Ok(format!("{}/graphql", config.url))
}

/// Get the package endpoint of the Brane API.
pub fn get_packages_endpoint() -> Result<String> {
    let config_file = get_config_dir().unwrap().join("registry.yml");
    let config = RegistryConfig::from_path(&config_file)
        .with_context(|| "No registry configuration found, please use `brane login` first.")?;

    Ok(format!("{}/packages", config.url))
}

///
///
///
pub fn login(
    url: String,
    username: String,
) -> Result<()> {
    let url = Url::parse(&url).with_context(|| format!("Not a valid absolute URL: {}", url))?;

    let host = url
        .host_str()
        .with_context(|| format!("URL does not have a (valid) host: {}", url))?;

    /* TIM */
    // Added quick error handling
    let config_file = match get_config_dir() {
        Ok(dir)  => dir.join("registry.yml"),
        Err(err) => { panic!("{}", err); }
    };
    /*******/
    let mut config = if config_file.exists() {
        RegistryConfig::from_path(&config_file)?
    } else {
        RegistryConfig::default()
    };

    config.username = username;
    config.url = format!("{}://{}:{}", url.scheme(), host, url.port().unwrap_or(50051));

    // Write registry.yml to config directory
    fs::create_dir_all(&config_file.parent().unwrap())?;
    let mut buffer = File::create(config_file)?;
    write!(buffer, "{}", serde_yaml::to_string(&config)?)?;

    Ok(())
}

///
///
///
pub fn logout() -> Result<()> {
    let config_file = ensure_config_dir(false).unwrap().join("registry.yml");
    if config_file.exists() {
        fs::remove_file(config_file)?;
    }

    Ok(())
}

///
///
///
pub async fn pull(
    name: String,
    version: Version,
) -> Result<()> {
    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "src/graphql/api_schema.json",
        query_path = "src/graphql/get_package.graphql",
        response_derives = "Debug"
    )]
    pub struct GetPackage;

    let package_dir = get_package_dir(&name, Some(&version))?;
    let mut temp_file = tempfile::NamedTempFile::new().expect("Failed to create temporary file.");

    let url = format!("{}/{}/{}", get_packages_endpoint()?, name, version);
    let mut package_archive = reqwest::get(&url).await?;
    let content_length = package_archive
        .headers()
        .get("content-length")
        .unwrap()
        .to_str()?
        .parse()?;

    // Write package archive to temporary file
    let progress = ProgressBar::new(content_length);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("Downloading... [{elapsed_precise}] {bar:40.cyan/blue} {percent}/100%")
            .progress_chars("##-"),
    );

    while let Some(chunk) = package_archive.chunk().await? {
        progress.inc(chunk.len() as u64);
        temp_file.write_all(&chunk)?;
    }

    progress.finish();

    // Copy package to package directory.
    fs::create_dir_all(&package_dir)?;
    fs::copy(temp_file.path(), package_dir.join("image.tar"))?;

    // Retreive package information from API.
    let client = reqwest::Client::new();
    let graphql_endpoint = get_graphql_endpoint()?;

    // Prepare GraphQL query.
    let variables = get_package::Variables {
        name: name.clone(),
        version: version.to_string(),
    };
    let graphql_query = GetPackage::build_query(variables);

    // Request/response for GraphQL query.
    let graphql_response = client.post(graphql_endpoint).json(&graphql_query).send().await?;
    let graphql_response: Response<get_package::ResponseData> = graphql_response.json().await?;

    if let Some(data) = graphql_response.data {
        let package = data.packages.first().expect("No package information available");
        let functions = package
            .functions_as_json
            .as_ref()
            .map(|f| serde_json::from_str(f).unwrap());

        let types = package.types_as_json.as_ref().map(|t| serde_json::from_str(t).unwrap());
        /* TIM */
        // TODO: Fix error handling
        let kind = PackageKind::from_str(&package.kind).unwrap();
        /*******/

        let package_info = PackageInfo {
            created: package.created,
            description: package.description.clone().unwrap_or_default(),
            detached: package.detached,
            digest: package.digest.clone(),
            functions: functions.unwrap_or_default(),
            id: package.id,
            kind,
            name: package.name.clone(),
            owners: package.owners.clone(),
            types: types.unwrap_or_default(),
            version: Version::from_str(&package.version)?,
        };

        // Write package.yml to package directory
        let mut buffer = File::create(package_dir.join("package.yml"))?;
        write!(buffer, "{}", serde_yaml::to_string(&package_info)?)?;
    } else {
        bail!("Failed to get package information from API.");
    }

    println!(
        "\nSuccessfully pulled version {} of package {}.",
        style(&version).bold().cyan(),
        style(&name).bold().cyan(),
    );

    Ok(())
}

/* TIM */
/// **Edited: the version is now optional.**
/// 
/// Pushes the given package to the remote instance that we're currently logged into.
/// 
/// **Arguments**
///  * `name`: The name/ID of the package to push.
///  * `version`: Optional package version to push. Will resolve it if it's the latest version.
/// 
/// **Returns**  
/// Nothing on success, or an anyhow error on failure.
pub async fn push(
    name: String,
    version: Version,
) -> Result<()> {
    // Try to get the general package directory
    let packages_dir = ensure_packages_dir(false)?;
    debug!("Using Brane package directory: {}", packages_dir.display());

    // Add the package name to the general directory
    let package_dir = packages_dir.join(&name);

    // Resolve the version number
    let version = if version.is_latest() {
        // Get the list of versions
        let mut versions = get_package_versions(&name, &package_dir)?;

        // Sort the versions and return the last one
        versions.sort();
        versions[versions.len() - 1].clone()
    } else {
        // Simply use the version given
        version.clone()
    };

    // Construct the full package directory with version
    let package_dir = ensure_package_dir(&name, Some(&version), false)?;
    let temp_file = tempfile::NamedTempFile::new().expect("Failed to create temporary file.");

    let progress = ProgressBar::new(0);
    progress.set_style(ProgressStyle::default_bar().template("Compressing... [{elapsed_precise}]"));
    progress.enable_steady_tick(250);

    // Create package tarball
    let gz = GzEncoder::new(&temp_file, Compression::fast());
    let mut tar = tar::Builder::new(gz);
    tar.append_dir_all(".", package_dir)?;
    tar.into_inner()?;

    progress.finish();

    // Upload file
    let url = get_packages_endpoint()?;
    let request = Client::new().post(&url);

    let progress = ProgressBar::new(0);
    progress.set_style(ProgressStyle::default_bar().template("Uploading...   [{elapsed_precise}]"));
    progress.enable_steady_tick(250);

    /* TIM */
    let file_handle = TokioFile::open(&temp_file).await;
    if let Err(reason) = file_handle {
        let code = reason.raw_os_error().unwrap_or(-1);
        eprintln!("Could not re-open temporary file '{}' as TokioFile: {}.", temp_file.path().to_string_lossy(), reason);
        std::process::exit(code);
    }
    // let file = TokioFile::open(&temp_file).await?;
    // let file = FramedRead::new(file, BytesCodec::new());
    let file = FramedRead::new(file_handle.ok().unwrap(), BytesCodec::new());
    /*******/

    let content_length = temp_file.path().metadata().unwrap().len();
    let request = request
        .body(Body::wrap_stream(file))
        .header("Content-Type", "application/gzip")
        .header("Content-Length", content_length);

    let response = request.send().await?;
    let response_status = response.status();

    progress.finish();

    if response_status.is_success() {
        println!(
            "\nSuccessfully pushed version {} of package {}.",
            style(&version).bold().cyan(),
            style(&name).bold().cyan(),
        );
    } else {
        let response_text = response.text().await?;
        println!("\nFailed to push package: {}", response_text)
    }

    Ok(())
}
/*******/

///
///
///
pub async fn search(term: Option<String>) -> Result<()> {
    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "src/graphql/api_schema.json",
        query_path = "src/graphql/search_packages.graphql",
        response_derives = "Debug"
    )]
    pub struct SearchPackages;

    let client = reqwest::Client::new();
    let graphql_endpoint = get_graphql_endpoint()?;

    // Prepare GraphQL query.
    let variables = search_packages::Variables { term };
    let graphql_query = SearchPackages::build_query(variables);

    // Request/response for GraphQL query.
    let graphql_response = client.post(graphql_endpoint).json(&graphql_query).send().await?;
    let graphql_response: Response<search_packages::ResponseData> = graphql_response.json().await?;

    if let Some(data) = graphql_response.data {
        let packages = data.packages;

        // Present results in a table.
        let format = FormatBuilder::new()
            .column_separator('\0')
            .borders('\0')
            .padding(1, 1)
            .build();

        let mut table = Table::new();
        table.set_format(format);
        table.add_row(row!["NAME", "VERSION", "KIND", "DESCRIPTION"]);

        for package in packages {
            let name = pad_str(&package.name, 20, Alignment::Left, Some(".."));
            let version = pad_str(&package.version, 10, Alignment::Left, Some(".."));
            let kind = pad_str(&package.kind, 10, Alignment::Left, Some(".."));
            let description = package.description.clone().unwrap_or_default();
            let description = pad_str(&description, 50, Alignment::Left, Some(".."));

            table.add_row(row![name, version, kind, description]);
        }

        table.printstd();
    } else {
        eprintln!("{:?}", graphql_response.errors);
    };

    Ok(())
}

///
///
///
pub async fn unpublish(
    name: String,
    version: Version,
    force: bool,
) -> Result<()> {
    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "src/graphql/api_schema.json",
        query_path = "src/graphql/unpublish_package.graphql",
        response_derives = "Debug"
    )]
    pub struct UnpublishPackage;

    let client = reqwest::Client::new();
    let graphql_endpoint = get_graphql_endpoint()?;

    // Ask for permission, if --force is not provided
    if !force {
        println!("Do you want to remove the following version(s)?");
        println!("- {}", version);

        // Abort, if not approved
        if !Confirm::new().interact()? {
            return Ok(());
        }

        println!();
    }

    // Prepare GraphQL query.
    if version.is_latest() { return Err(anyhow!("Cannot unpublish 'latest' package version; choose a version.")); }
    let variables = unpublish_package::Variables { name, version: version.to_string() };
    let graphql_query = UnpublishPackage::build_query(variables);

    // Request/response for GraphQL query.
    let graphql_response = client.post(graphql_endpoint).json(&graphql_query).send().await?;
    let graphql_response: Response<unpublish_package::ResponseData> = graphql_response.json().await?;

    if let Some(data) = graphql_response.data {
        println!("{}", data.unpublish_package);
    } else {
        eprintln!("{:?}", graphql_response.errors);
    };

    Ok(())
}
