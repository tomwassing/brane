use std::str::FromStr;

use anyhow::Result;
use chrono::{DateTime, Utc};
use graphql_client::{GraphQLQuery, Response};
use reqwest::Client;
use uuid::Uuid;

use specifications::package::{PackageKind, PackageIndex, PackageInfo};
use specifications::version::Version;


type DateTimeUtc = DateTime<Utc>;

///
///
///
pub async fn get_package_index(graphql_endpoint: &str) -> Result<PackageIndex> {
    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "src/graphql/api_schema.json",
        query_path = "src/graphql/get_packages.graphql",
        response_derives = "Debug"
    )]
    pub struct GetPackages;

    let client = Client::new();

    // Prepare GraphQL query.
    let variables = get_packages::Variables {};
    let graphql_query = GetPackages::build_query(variables);

    // Request/response for GraphQL query.
    let graphql_response = client.post(graphql_endpoint).json(&graphql_query).send().await?;
    let graphql_response: Response<get_packages::ResponseData> = graphql_response.json().await?;

    let packages = graphql_response
        .data
        .expect("Expecting zero or more packages.")
        .packages;
    let packages = packages
        .into_iter()
        .map(|p| {
            let functions = p.functions_as_json.map(|f| serde_json::from_str(&f).unwrap());
            let types = p.types_as_json.map(|t| serde_json::from_str(&t).unwrap());
            // TODO: Return properly
            let kind = PackageKind::from_str(&p.kind).unwrap();

            let version = p.version.clone();
            PackageInfo {
                created: p.created,
                description: p.description.unwrap_or_default(),
                detached: p.detached,
                digest: p.digest,
                functions: functions.unwrap_or_default(),
                id: p.id,
                kind,
                name: p.name,
                owners: p.owners,
                types: types.unwrap_or_default(),
                version: Version::from_str(&version).unwrap_or_else(|err| panic!("Could not parse GraphQL-obtained package version '{}': {}", &version, err)),
            }
        })
        .collect();

    // TODO: Fix error handling
    PackageIndex::from_packages(packages).map_err(|e| anyhow!(e))
}
