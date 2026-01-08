use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredService {
    pub service_type: String,
    pub fullname: String,
    pub hostname: String,
    pub port: u16,
    pub ips: Vec<String>,
    pub txt_records: HashMap<String, String>,
    pub last_seen: u64,
}

#[derive(Clone)]
pub struct DiscoveryState {
    services: Arc<RwLock<HashMap<String, DiscoveredService>>>,
}

impl DiscoveryState {
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_services(&self) -> Vec<DiscoveredService> {
        let services = self.services.read().unwrap();
        services.values().cloned().collect()
    }

    fn update_service(&self, service: DiscoveredService) {
        let mut services = self.services.write().unwrap();
        services.insert(service.fullname.clone(), service);
    }

    fn remove_service(&self, fullname: &str) {
        let mut services = self.services.write().unwrap();
        services.remove(fullname);
    }
}

pub fn start_discovery(state: DiscoveryState) {
    tokio::task::spawn_blocking(move || {
        let mdns = ServiceDaemon::new().expect("Failed to create mDNS daemon");
        let _receiver = mdns
            .browse("_services._dns-sd._udp.local.")
            .expect("Failed to browse services");

        info!("Starting mDNS discovery loop...");

        let type_receiver = mdns
            .browse("_services._dns-sd._udp.local.")
            .expect("Failed to start type browsing");

        let mut browsing_types = std::collections::HashSet::new();

        loop {
            while let Ok(event) = type_receiver.recv() {
                match event {
                    ServiceEvent::ServiceFound(service_type, fullname) => {
                        info!(
                            "Found service type candidate: {} (type: {})",
                            fullname, service_type
                        );

                        let type_to_browse = fullname.clone();

                        if browsing_types.contains(&type_to_browse) {
                            continue;
                        }
                        browsing_types.insert(type_to_browse.clone());

                        info!("Browsing for instances of type: {}", type_to_browse);

                        if let Ok(service_receiver) = mdns.browse(&type_to_browse) {
                            let state_clone = state.clone();
                            let type_name_clone = type_to_browse.clone();

                            std::thread::spawn(move || {
                                while let Ok(service_event) = service_receiver.recv() {
                                    match service_event {
                                        ServiceEvent::ServiceResolved(svc_info) => {
                                            info!("Resolved service: {}", svc_info.get_fullname());
                                            let timestamp = SystemTime::now()
                                                .duration_since(UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs();

                                            let ips: Vec<String> = svc_info
                                                .get_addresses()
                                                .iter()
                                                .map(|ip| ip.to_string())
                                                .collect();

                                            let mut txt = HashMap::new();
                                            let props = svc_info.get_properties();
                                            for prop in props.iter() {
                                                txt.insert(
                                                    prop.key().to_string(),
                                                    prop.val_str().to_string(),
                                                );
                                            }

                                            let ds = DiscoveredService {
                                                service_type: type_name_clone.clone(),
                                                fullname: svc_info.get_fullname().to_string(),
                                                hostname: svc_info.get_hostname().to_string(),
                                                port: svc_info.get_port(),
                                                ips,
                                                txt_records: txt,
                                                last_seen: timestamp,
                                            };

                                            state_clone.update_service(ds);
                                        }
                                        ServiceEvent::ServiceRemoved(type_name, fullname) => {
                                            info!("Service removed: {} ({})", fullname, type_name);
                                            state_clone.remove_service(&fullname);
                                        }
                                        _ => {}
                                    }
                                }
                            });
                        }
                    }
                    _e => {}
                }
            }
        }
    });
}
