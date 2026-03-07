use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use tracing::{info, warn};

use crate::BlockInfo;
use crate::config::MqttConfig;

/// MQTT publisher for block notifications.
/// Publishes BlockInfo JSON to `{topic_prefix}/{chain_id}/blocks` on each new block.
pub struct MqttPublisher {
    client: AsyncClient,
    topic_prefix: String,
}

impl MqttPublisher {
    /// Connect to the MQTT broker. Spawns a background task to drive the event loop.
    /// Returns None if connection setup fails (recorder continues without MQTT).
    pub fn connect(config: &MqttConfig) -> Option<Self> {
        let mut opts = MqttOptions::new(&config.client_id, &config.host, config.port);
        opts.set_keep_alive(std::time::Duration::from_secs(30));
        // Cap the channel to prevent unbounded memory growth if broker is slow.
        let (client, eventloop) = AsyncClient::new(opts, 64);

        // Spawn the event loop driver — it reconnects automatically on failure.
        tokio::spawn(drive_eventloop(eventloop));

        info!(
            host = %config.host,
            port = config.port,
            client_id = %config.client_id,
            "MQTT publisher connected"
        );

        Some(MqttPublisher {
            client,
            topic_prefix: config.topic_prefix.clone(),
        })
    }

    /// Publish a block notification. Non-blocking — logs warning on failure.
    pub async fn publish_block(&self, chain_id: &str, info: &BlockInfo) {
        let topic = format!("{}/{}/blocks", self.topic_prefix, chain_id);
        let payload = match serde_json::to_vec(info) {
            Ok(p) => p,
            Err(e) => {
                warn!("MQTT: failed to serialize BlockInfo: {}", e);
                return;
            }
        };

        if let Err(e) = self.client.publish(&topic, QoS::AtLeastOnce, false, payload).await {
            warn!("MQTT: publish to {} failed: {}", topic, e);
        }
    }
}

/// Drive the rumqttc event loop. Logs errors but never stops — rumqttc
/// handles reconnection internally.
async fn drive_eventloop(mut eventloop: EventLoop) {
    loop {
        match eventloop.poll().await {
            Ok(_event) => {} // connection events, acks, etc.
            Err(e) => {
                warn!("MQTT event loop error (will retry): {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}
