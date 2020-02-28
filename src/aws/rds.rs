use {
    crate::aws::cloudwatch::CwClient,
    crate::aws::util,
    crate::aws::Result,
    crate::config::{RdsConfig, RequiredTags},
    crate::service::{EnforcementState, NTag, NukeService, Resource, ResourceType},
    log::debug,
    rusoto_core::{HttpClient, Region},
    rusoto_credential::ProfileProvider,
    rusoto_rds::{
        DBInstance, DeleteDBInstanceMessage, DescribeDBInstancesMessage, Filter,
        ListTagsForResourceMessage, ModifyDBInstanceMessage, Rds, RdsClient, StopDBInstanceMessage,
        Tag,
    },
};

const AURORA_POSTGRES_ENGINE: &str = "aurora-postgresql";
const AURORA_MYSQL_ENGINE: &str = "aurora-mysql";

pub struct RdsNukeClient {
    pub client: RdsClient,
    pub cwclient: CwClient,
    pub config: RdsConfig,
    pub region: Region,
    pub dry_run: bool,
}

impl RdsNukeClient {
    pub fn new(
        profile_name: Option<&str>,
        region: Region,
        config: RdsConfig,
        dry_run: bool,
    ) -> Result<Self> {
        if let Some(profile) = profile_name {
            let mut pp = ProfileProvider::new()?;
            pp.set_profile(profile);
            Ok(RdsNukeClient {
                client: RdsClient::new_with(HttpClient::new()?, pp, region.clone()),
                cwclient: CwClient::new(profile_name, region.clone(), config.clone().idle_rules)?,
                config,
                region,
                dry_run,
            })
        } else {
            Ok(RdsNukeClient {
                client: RdsClient::new(region.clone()),
                cwclient: CwClient::new(profile_name, region.clone(), config.clone().idle_rules)?,
                config,
                region,
                dry_run,
            })
        }
    }

    fn package_tags_as_ntags(&self, tags: Option<Vec<Tag>>) -> Option<Vec<NTag>> {
        tags.map(|ts| {
            ts.iter()
                .map(|tag| NTag {
                    key: tag.key.clone(),
                    value: tag.value.clone(),
                })
                .collect()
        })
    }

    fn package_instances_as_resources(&self, instances: Vec<DBInstance>) -> Result<Vec<Resource>> {
        let mut resources: Vec<Resource> = Vec::new();

        for instance in instances {
            let instance_id = instance.db_instance_identifier.as_ref().unwrap().to_owned();

            let enforcement_state: EnforcementState = {
                if self.config.ignore.contains(&instance_id) {
                    EnforcementState::SkipConfig
                } else if instance.db_instance_status != Some("available".to_string()) {
                    EnforcementState::SkipStopped
                } else {
                    if self.resource_tags_does_not_match(&instance) {
                        EnforcementState::from_target_state(&self.config.target_state)
                    } else if self.resource_types_does_not_match(&instance) {
                        EnforcementState::from_target_state(&self.config.target_state)
                    } else if self.is_resource_idle(&instance) {
                        EnforcementState::from_target_state(&self.config.target_state)
                    } else {
                        EnforcementState::Skip
                    }
                }
            };

            resources.push(Resource {
                id: instance_id,
                region: self.region.clone(),
                resource_type: ResourceType::RDS,
                tags: self.package_tags_as_ntags(self.list_tags(instance.db_instance_arn.clone())?),
                state: instance.db_instance_status.clone(),
                enforcement_state,
            });
        }

        Ok(resources)
    }

    fn resource_tags_does_not_match(&self, instance: &DBInstance) -> bool {
        if !self.config.required_tags.is_empty() {
            !self.check_tags(
                &self
                    .list_tags(instance.db_instance_arn.clone())
                    .unwrap_or_default(),
                &self.config.required_tags,
            )
        } else {
            false
        }
    }

    fn resource_types_does_not_match(&self, instance: &DBInstance) -> bool {
        if !self.config.allowed_instance_types.is_empty() {
            !self
                .config
                .allowed_instance_types
                .contains(&instance.db_instance_class.clone().unwrap())
        } else {
            false
        }
    }

    fn is_resource_idle(&self, instance: &DBInstance) -> bool {
        if !self.config.idle_rules.is_empty() {
            !self
                .cwclient
                .filter_db_instance(&instance.db_instance_identifier.as_ref().unwrap())
                .unwrap()
        } else {
            false
        }
    }

    fn get_instances(&self, filter: Vec<Filter>) -> Result<Vec<DBInstance>> {
        let mut next_token: Option<String> = None;
        let mut instances: Vec<DBInstance> = Vec::new();

        loop {
            let result = self
                .client
                .describe_db_instances(DescribeDBInstancesMessage {
                    filters: Some(filter.clone()),
                    marker: next_token,
                    ..Default::default()
                })
                .sync()?;

            if let Some(db_instances) = result.db_instances {
                let mut temp_instances: Vec<DBInstance> = db_instances
                    .into_iter()
                    .filter(|i| {
                        i.engine != Some(AURORA_MYSQL_ENGINE.into())
                            && i.engine != Some(AURORA_POSTGRES_ENGINE.into())
                    })
                    .collect();

                instances.append(&mut temp_instances);
            }

            if result.marker.is_none() {
                break;
            } else {
                next_token = result.marker;
            }
        }

        Ok(instances)
    }

    fn list_tags(&self, arn: Option<String>) -> Result<Option<Vec<Tag>>> {
        let result = self
            .client
            .list_tags_for_resource(ListTagsForResourceMessage {
                resource_name: arn.unwrap(),
                ..Default::default()
            })
            .sync()?;
        Ok(result.tag_list)
    }

    fn check_tags(&self, tags: &Option<Vec<Tag>>, required_tags: &Vec<RequiredTags>) -> bool {
        let ntags = self.package_tags_as_ntags(tags.to_owned());
        util::compare_tags(ntags, required_tags)
    }

    fn disable_termination_protection(&self, instance_id: &str) -> Result<()> {
        // TODO: This call can be saved by saving the termiantion protection state in the
        // Resource struct, while scanning for isntances.
        let resp = self
            .client
            .describe_db_instances(DescribeDBInstancesMessage {
                db_instance_identifier: Some(instance_id.to_owned()),
                ..Default::default()
            })
            .sync()?;

        if resp.db_instances.is_some() {
            if resp
                .db_instances
                .unwrap()
                .first()
                .unwrap()
                .deletion_protection
                == Some(true)
            {
                debug!(
                    "Termination protection is enabled for: {}. Trying to disable it.",
                    instance_id
                );

                if !self.dry_run {
                    self.client
                        .modify_db_instance(ModifyDBInstanceMessage {
                            db_instance_identifier: instance_id.to_owned(),
                            deletion_protection: Some(false),
                            ..Default::default()
                        })
                        .sync()?;
                }
            }
        }

        Ok(())
    }

    fn terminate_resource(&self, instance_id: String) -> Result<()> {
        debug!("Terminating instance: {:?}", instance_id);

        if !self.dry_run {
            if self.config.termination_protection.ignore {
                self.disable_termination_protection(&instance_id)?;
            }

            self.client
                .delete_db_instance(DeleteDBInstanceMessage {
                    db_instance_identifier: instance_id,
                    delete_automated_backups: Some(false),
                    ..Default::default()
                })
                .sync()?;
        }

        Ok(())
    }

    fn stop_resource(&self, instance_id: String) -> Result<()> {
        debug!("Stopping instance: {:?}", instance_id);

        if !self.dry_run {
            self.client
                .stop_db_instance(StopDBInstanceMessage {
                    db_instance_identifier: instance_id,
                    ..Default::default()
                })
                .sync()?;
        }

        Ok(())
    }
}

impl NukeService for RdsNukeClient {
    fn scan(&self) -> Result<Vec<Resource>> {
        let instances = self.get_instances(Vec::new())?;

        Ok(self.package_instances_as_resources(instances)?)
    }

    fn stop(&self, resource: &Resource) -> Result<()> {
        self.stop_resource(resource.id.to_owned())
    }

    fn delete(&self, resource: &Resource) -> Result<()> {
        self.terminate_resource(resource.id.to_owned())
    }

    fn as_any(&self) -> &dyn ::std::any::Any {
        self
    }
}
