use std::collections::HashMap;

use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS, Event, Packet};
use rumqttd::{Broker, Config as BrokerConfig, ConnectionSettings, ServerSettings};
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Start an embedded MQTT broker on the given port. Returns the port actually bound.
pub fn start_broker(port: u16) -> u16 {
    let router_config = rumqttd::RouterConfig {
        max_segment_size: 10 * 1024 * 1024, // 10MB segments
        max_connections: 100,
        ..Default::default()
    };

    let server_settings = ServerSettings {
        name: "sims".to_string(),
        listen: format!("0.0.0.0:{}", port).parse().unwrap(),
        tls: None,
        next_connection_delay_ms: 1,
        connections: ConnectionSettings {
            connection_timeout_ms: 5000,
            max_payload_size: 65536,
            max_inflight_count: 100,
            auth: None,
            external_auth: None,
            dynamic_filters: false,
        },
    };

    let mut servers = HashMap::new();
    servers.insert("sims".to_string(), server_settings);

    let broker_config = BrokerConfig {
        id: 0,
        router: router_config,
        v4: Some(servers),
        v5: None,
        ws: None,
        cluster: None,
        console: None,
        bridge: None,
        prometheus: None,
        metrics: None,
    };

    let mut broker = Broker::new(broker_config);
    // Spawn broker in a background thread (it's blocking)
    std::thread::spawn(move || {
        broker.start().expect("MQTT broker failed to start");
    });

    // Wait for broker to accept connections (up to 5s)
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let addr = format!("127.0.0.1:{}", port);
    while std::time::Instant::now() < deadline {
        if std::net::TcpStream::connect(&addr).is_ok() {
            info!("Embedded MQTT broker started on port {}", port);
            return port;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    warn!("MQTT broker may not be ready on port {} after 5s timeout", port);
    port
}

/// Connect the ao-recorder's MQTT publisher to the embedded broker.
pub fn connect_recorder_publisher(
    broker_port: u16,
    state: &ao_recorder::AppState,
) {
    let config = ao_recorder::config::MqttConfig {
        host: "127.0.0.1".to_string(),
        port: broker_port,
        client_id: "ao-sims-recorder".to_string(),
        topic_prefix: "ao/chain".to_string(),
    };

    if let Some(publisher) = ao_recorder::mqtt::MqttPublisher::connect(&config) {
        state.set_mqtt(publisher);
        info!("Recorder MQTT publisher connected to embedded broker");
    } else {
        warn!("Failed to connect recorder MQTT publisher");
    }
}

/// Block notification received from MQTT.
#[derive(Debug, Clone)]
pub struct BlockNotification {
    pub chain_id: String,
    pub height: u64,
}

/// Subscribe to block notifications from the MQTT broker.
/// Returns a channel that receives notifications for all chains.
pub async fn subscribe_blocks(
    broker_port: u16,
    client_id: &str,
) -> mpsc::Receiver<BlockNotification> {
    let (tx, rx) = mpsc::channel(256);

    let mut opts = MqttOptions::new(client_id, "127.0.0.1", broker_port);
    opts.set_keep_alive(std::time::Duration::from_secs(30));
    let (client, eventloop) = AsyncClient::new(opts, 64);

    // Subscribe to all chain block topics
    client.subscribe("ao/chain/+/blocks", QoS::AtLeastOnce).await
        .unwrap_or_else(|e| warn!("MQTT subscribe failed: {}", e));

    tokio::spawn(drive_subscriber(eventloop, tx));

    rx
}

async fn drive_subscriber(
    mut eventloop: EventLoop,
    tx: mpsc::Sender<BlockNotification>,
) {
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(publish))) => {
                // Topic format: ao/chain/{chain_id}/blocks
                let topic: &str = &publish.topic;
                let parts: Vec<&str> = topic.split('/').collect();
                if parts.len() >= 3 {
                    let chain_id = parts[2].to_string();
                    // Parse payload to get block height
                    if let Ok(info) = serde_json::from_slice::<serde_json::Value>(&publish.payload) {
                        let height = info["height"].as_u64().unwrap_or(0);
                        let _ = tx.send(BlockNotification { chain_id, height }).await;
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                warn!("MQTT subscriber error (will retry): {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    }
}
