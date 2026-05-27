+++
title = "Database Failover"
audience = "sre"
last_reviewed = "2026-05-18"
+++

# Database Failover

Use this runbook when the primary payments database becomes unresponsive
or replication lag exceeds the alert threshold.

## Pre-flight

Confirm the standby is healthy before initiating failover. The standby
health dashboard is the source of truth:

![standby health dashboard](https://docs.acme.test/img/standby-health.png)

If the standby is unhealthy, escalate before attempting failover.

## Procedure

1. Quiesce write traffic at the load balancer.
2. Wait for in-flight transactions to drain. The drain window is bounded
   by the configured statement timeout.
3. Promote the standby. The runbook for the promotion command lives in
   the platform docs at [platform/db-promote](./missing-platform-link).
4. Re-enable write traffic.
5. Verify the application logs and synthetic checks before opening the
   incident bridge for closure.

## After action

Open a post-incident review within twenty-four hours. The review
template is linked from the post-incident wiki.
