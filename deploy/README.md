# 📊 Monitoring Infrastructure

This directory contains the infrastructure configuration for monitoring the **ARB-BOT** trading engine. It provides a
pre-configured stack consisting of **Prometheus** for metrics collection and **Grafana** for real-time visualization.

## 🏗 Directory Structure

- `docker-compose.yml` — Orchestrates the monitoring services.
- `grafana/provisioning/` — Automated configuration for data sources and dashboards.
- `grafana/dashboards/` — Pre-configured JSON dashboard models for HFT metrics.
- `grafana/prometheus.yml` — Scraper configuration for metric collection targets.

## 🚀 Quick Start

### 1. Launch the Stack

Run the following command from the project root directory:

```bash
docker-compose -f deploy/docker-compose.yml up -d
```

### 2. Access Interfaces

Once the containers are initialized, the services will be available at:

* **Grafana**: http://localhost:3000

    * **Default Login**: admin
    * **Default Password**: admin

* **Prometheus**: http://localhost:9090

## 🔧 Bot Configuration

To ensure metrics are correctly scraped by Prometheus, verify that your config.toml has the observability server
configured correctly:

```toml
[general]
server_addr = "127.0.0.1:9000"
metrics_addr = "127.0.0.1:9007" # Prometheus is configured to scrape this port
```

**Disclaimer**: Ensure ports 3000 and 9090 are available on your host system before starting the stack.
