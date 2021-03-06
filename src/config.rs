//! Configuration Parser
use crate::client::Client;
use clap::{App, Arg};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::{fmt, fs::File, io::Read, str::FromStr, time::Duration};
use tracing::warn;

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

pub type Config = HashMap<Client, ResourceConfig>;

/// Cli Args
#[derive(Debug, Clone)]
pub struct Args {
    pub config: String,
    pub profile: Option<String>,
    pub regions: Vec<String>,
    pub targets: Option<Vec<Client>>,
    pub exclude: Option<Vec<Client>>,
    pub dry_run: bool,
    pub force: bool,
    pub verbose: u64,
    pub version: String,
}

/// Configuration struct for nuker executable
///
/// This struct is built from reading the configuration file
#[derive(Debug, Deserialize, Clone)]
pub struct ParsedConfig {
    #[serde(default = "default_resource_config")]
    pub ec2_instance: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ec2_sg: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ec2_eni: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ec2_address: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ebs_volume: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ebs_snapshot: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub elb_alb: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub elb_nlb: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub rds_instance: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub rds_cluster: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub s3_bucket: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub emr_cluster: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub rs_cluster: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub glue_endpoint: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub sagemaker_notebook: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub es_domain: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub asg: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ecs_cluster: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub eks_cluster: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ec2_vpc: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ec2_igw: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ec2_nat_gw: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ec2_network_acl: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ec2_peering_connection: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ec2_rt: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ec2_subnet: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ec2_vpc_endpoint: ResourceConfig,
    #[serde(default = "default_resource_config")]
    pub ec2_vpn_gw: ResourceConfig,
}

#[derive(Debug, Deserialize, Clone, Eq, PartialEq)]
pub enum TargetState {
    Stopped,
    Deleted,
}

impl Default for TargetState {
    fn default() -> Self {
        TargetState::Deleted
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RequiredTag {
    pub name: String,
    pub pattern: Option<String>,
    #[serde(skip)]
    pub regex: Option<Regex>,
}

#[derive(Debug, Deserialize, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub enum FilterOp {
    Lt,
    Gt,
    Le,
    Ge,
}

impl Default for FilterOp {
    fn default() -> Self {
        FilterOp::Lt
    }
}

#[derive(Debug, Deserialize, Copy, Clone)]
pub enum MetricStatistic {
    SampleCount,
    Average,
    Sum,
    Minimum,
    Maximum,
}

impl Default for MetricStatistic {
    fn default() -> Self {
        MetricStatistic::Average
    }
}

impl fmt::Display for MetricStatistic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct MetricFilter {
    pub name: String,
    pub statistic: MetricStatistic,
    #[serde(with = "humantime_serde")]
    pub duration: Duration,
    #[serde(with = "humantime_serde")]
    pub period: Duration,
    pub op: FilterOp,
    pub dimensions: Option<Vec<MetricDimension>>,
    pub value: f32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MetricDimension {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TerminationProtection {
    pub ignore: bool,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ManageStopped {
    #[serde(with = "humantime_serde")]
    pub older_than: Duration,
    #[serde(skip)]
    pub dt_extract_regex: Option<Regex>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct NamingPrefix {
    pub pattern: String,
    #[serde(skip)]
    pub regex: Option<Regex>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResourceConfig {
    #[serde(default)]
    pub target_state: TargetState,
    #[serde(default)]
    pub required_tags: Option<Vec<RequiredTag>>,
    #[serde(default)]
    pub allowed_types: Option<Vec<String>>,
    #[serde(default)]
    pub whitelist: Option<Vec<String>>,
    #[serde(default)]
    pub metric_filters: Option<Vec<MetricFilter>>,
    #[serde(default)]
    pub termination_protection: Option<TerminationProtection>,
    #[serde(default)]
    pub manage_stopped: Option<ManageStopped>,
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    pub max_run_time: Option<Duration>,
    #[serde(default)]
    pub disable_additional_rules: bool,
    #[serde(default)]
    pub naming_prefix: Option<NamingPrefix>,
}

impl Default for ResourceConfig {
    fn default() -> Self {
        ResourceConfig {
            target_state: TargetState::Deleted,
            required_tags: None,
            allowed_types: None,
            whitelist: None,
            metric_filters: None,
            termination_protection: Some(TerminationProtection { ignore: true }),
            manage_stopped: None,
            max_run_time: None,
            disable_additional_rules: false,
            naming_prefix: None,
        }
    }
}

/// Parse the command line arguments for nuker executable
pub fn parse_args() -> Args {
    let args = App::new("nuker")
        .about("Cleans up AWS resources based on configurable Rules.")
        .version(VERSION.unwrap_or("unknown"))
        .subcommand(App::new("resource-types").about("Prints out supported resource types"))
        .arg(
            Arg::with_name("config-file")
                .long("config")
                .short("C")
                .value_name("config")
                // .required(true)
                .help("The config file to feed in.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("region")
                .long("region")
                .help(
                    "Which regions to enforce the rules in. Default is the rules will be \
                    enforced across all the regions.",
                )
                .takes_value(true)
                .multiple(true)
                .number_of_values(1),
        )
        .arg(
            Arg::with_name("target")
                .long("target")
                .help(
                    "Services to include from rules enforcement. This will take precedence \
                over the configuration file.",
                )
                .takes_value(true)
                .multiple(true)
                .number_of_values(1),
        )
        .arg(
            Arg::with_name("exclude")
                .long("exclude")
                .help(
                    "Services to exclude from rules enforcement. This will take precedence \
                over the configuration file.",
                )
                .takes_value(true)
                .multiple(true)
                .number_of_values(1)
                .conflicts_with("target"),
        )
        .arg(
            Arg::with_name("profile")
                .long("profile")
                .help(
                    "Named Profile to use for authenticating with AWS. If the profile is \
                    *not* set, the credentials will be sourced in the following order: \n\
                    1. Environment variables: AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY \n\
                    2. AWS credentials file. Usually located at ~/.aws/credentials.\n\
                    3. IAM instance profile",
                )
                .takes_value(true),
        )
        .arg(Arg::with_name("no-dry-run").long("no-dry-run").help(
            "Disables the dry run behavior, which just lists the resources that are \
                    being cleaned but not actually delete them. Enabling this option will disable \
                    dry run behavior and deletes the resources.",
        ))
        .arg(
            Arg::with_name("force")
                .long("force")
                .help("Does not prompt for confirmation when dry run is disabled"),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .multiple(true)
                .help("Turn on verbose output."),
        )
        .get_matches();

    if let Some(ref _matches) = args.subcommand_matches("resource-types") {
        for r in Client::iter() {
            print!("{} ", r.name());
        }
        ::std::process::exit(0);
    }

    if !args.is_present("config-file") {
        panic!("--config <config> is a required parameter");
    }

    let verbose = if args.is_present("verbose") {
        args.occurrences_of("verbose")
    } else {
        0
    };

    let dry_run = if args.is_present("no-dry-run") {
        false
    } else {
        true
    };

    let force = if args.is_present("force") {
        true
    } else {
        false
    };

    let regions: Vec<&str> = if args.is_present("region") {
        args.values_of("region").unwrap().collect()
    } else {
        vec![]
    };

    let targets: Option<Vec<Client>> = if args.is_present("target") {
        Some(
            args.values_of("target")
                .unwrap()
                .map(|t| Client::from_str(t).unwrap())
                .collect(),
        )
    } else {
        None
    };

    let exclude: Option<Vec<Client>> = if args.is_present("exclude") {
        Some(
            args.values_of("exclude")
                .unwrap()
                .map(|e| Client::from_str(e).unwrap())
                .collect(),
        )
    } else {
        None
    };

    Args {
        config: args.value_of("config-file").unwrap().to_string(),
        regions: regions.iter().map(|r| r.to_string()).collect(),
        profile: args.value_of("profile").map(|s| s.to_owned()),
        targets,
        exclude,
        dry_run,
        force,
        verbose,
        version: VERSION.unwrap_or("unknown").to_string(),
    }
}

/// Parses the nuker configuration file
pub fn parse_config_file(filename: &str) -> Config {
    let mut fp = match File::open(filename) {
        Err(e) => panic!("Could not open file {} with error {}", filename, e),
        Ok(fp) => fp,
    };

    let mut buffer = String::new();
    fp.read_to_string(&mut buffer).unwrap();
    parse_config(&buffer)
}

pub fn parse_config(buffer: &str) -> Config {
    let config: ParsedConfig = toml::from_str(buffer).expect("could not parse toml configuration");
    let mut config_map: HashMap<Client, ResourceConfig> = HashMap::new();

    config_map.insert(Client::Asg, config.asg);
    config_map.insert(Client::Ec2Instance, config.ec2_instance);
    config_map.insert(Client::Ec2Sg, config.ec2_sg);
    config_map.insert(Client::Ec2Eni, config.ec2_eni);
    config_map.insert(Client::Ec2Address, config.ec2_address);
    config_map.insert(Client::EbsVolume, config.ebs_volume);
    config_map.insert(Client::Ec2Vpc, config.ec2_vpc);
    config_map.insert(Client::EbsSnapshot, config.ebs_snapshot);
    config_map.insert(Client::RdsInstance, config.rds_instance);
    config_map.insert(Client::RdsCluster, config.rds_cluster);
    config_map.insert(Client::EcsCluster, config.ecs_cluster);
    config_map.insert(Client::ElbAlb, config.elb_alb);
    config_map.insert(Client::ElbNlb, config.elb_nlb);
    config_map.insert(Client::EmrCluster, config.emr_cluster);
    config_map.insert(Client::EsDomain, config.es_domain);
    config_map.insert(Client::GlueEndpoint, config.glue_endpoint);
    config_map.insert(Client::RsCluster, config.rs_cluster);
    config_map.insert(Client::SagemakerNotebook, config.sagemaker_notebook);
    config_map.insert(Client::S3Bucket, config.s3_bucket);
    config_map.insert(Client::Ec2Igw, config.ec2_igw);
    config_map.insert(Client::Ec2Subnet, config.ec2_subnet);
    config_map.insert(Client::Ec2RouteTable, config.ec2_rt);
    config_map.insert(Client::Ec2NetworkACL, config.ec2_network_acl);
    config_map.insert(Client::Ec2NatGW, config.ec2_nat_gw);
    config_map.insert(Client::Ec2VpnGW, config.ec2_vpn_gw);
    config_map.insert(Client::Ec2VpcEndpoint, config.ec2_vpc_endpoint);
    config_map.insert(Client::Ec2PeeringConnection, config.ec2_peering_connection);
    config_map.insert(Client::EksCluster, config.eks_cluster);

    // Compile all regex expressions up front
    for (_client, r_config) in &mut config_map {
        if let Some(req_tags) = r_config.required_tags.as_mut() {
            for rt in req_tags {
                if let Some(pattern) = rt.pattern.as_mut() {
                    rt.regex = compile_regex(pattern.as_str());
                }
            }
        }

        if let Some(naming_prefix) = r_config.naming_prefix.as_mut() {
            naming_prefix.regex = compile_regex(naming_prefix.pattern.as_str());
        }

        // if let Some(manage_stopped) = &mut config.ec2_instance.manage_stopped {
        //     manage_stopped.dt_extract_regex = compile_regex(r"^.*\((?P<datetime>.*)\)$");
        // }
    }

    config_map
}

fn compile_regex(pattern: &str) -> Option<Regex> {
    match Regex::new(pattern) {
        Ok(regex) => Some(regex),
        Err(err) => {
            warn!("Failed compiling regex: {} - {:?}", pattern, err);
            None
        }
    }
}

fn default_resource_config() -> ResourceConfig {
    ResourceConfig::default()
}
