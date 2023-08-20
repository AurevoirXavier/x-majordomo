// std
use std::{env, time::Duration};
// crates.io
use hap::serde_json::{self, Value};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use tokio::task;
// pdu
use crate::{prelude::*, OneshotTx, Rx};

#[derive(Debug)]
pub struct MqttMessage {
	pub topic: String,
	pub payload: String,
}

pub async fn start(mqtt_tx: OneshotTx, mut mqtt_rx: Rx) -> Result<()> {
	let mut mqtt_options = MqttOptions::new("rumqtt-async", env::var("MQTT_HOST").unwrap(), 1883);

	mqtt_options.set_keep_alive(Duration::from_secs(5));

	let (client, mut event_loop) = AsyncClient::new(mqtt_options, 10);

	client
		.subscribe("/yespeed/pdu/yespeed/19847786504205500130392/out/1000100", QoS::AtMostOnce)
		.await?;
	client
		.publish(
			"/yespeed/pdu/yespeed/19847786504205500130392/in/1000100",
			QoS::AtLeastOnce,
			false,
			"{}",
		)
		.await?;

	let state = loop {
		let Event::Incoming(Incoming::Publish(p)) = event_loop.poll().await? else { continue };

		// Trim prefix `"devices":`.
		break serde_json::from_slice::<Value>(&p.payload[10..])?;
	};

	mqtt_tx.send(state).map_err(|e| anyhow::anyhow!("{e}"))?;
	client.unsubscribe("/yespeed/pdu/yespeed/19847786504205500130392/in/1000100").await?;

	task::spawn(async move {
		loop {
			// err
			let Some(MqttMessage { topic, payload }) = mqtt_rx.recv().await else { continue };
			let _ = client.publish(topic, QoS::AtLeastOnce, false, payload).await;
		}
	});
	task::spawn(async move {
		loop {
			let Ok(Event::Incoming(Incoming::Publish(p))) = event_loop.poll().await else {
				continue;
			};

			tracing::info!("{}", String::from_utf8_lossy(&p.payload));
		}
	});

	Ok(())
}
