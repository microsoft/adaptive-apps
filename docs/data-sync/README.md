# Data Sync

To support business continuity during cloud disconnection, applications may need to synchronize data between cloud-based databases and local databases. Adaptive App does not pre-package a specific data synchronization solution in its capability portfolios because data sync is often platform-specific and depends heavily on business requirements, database technologies, consistency expectations, and conflict-resolution rules.

Instead, Adaptive App provides the following general guidance and recommends selecting the most appropriate synchronization approach for each scenario.

## Define explicit business continuity expectations

Business continuity can mean different things in different systems. In some cases, it may mean “operate as normal” while disconnected from the cloud. In other cases, it may mean operating with degraded capabilities.

For example, in a degraded mode, a local site may continue to record new customer orders, but the orders may be queued for processing until the cloud connection is restored. In another scenario, the system may allow read-only access to local data but block new transactions.

These expectations should be defined explicitly because they directly affect the system design, data ownership model, conflict-resolution requirements, and technology choices.

## Prefer one-way synchronization when possible

Two-way synchronization is usually more complex because it can introduce write-write conflicts, ordering issues, and reconciliation challenges. When possible, prefer one-way synchronization channels.

This does not mean data can only flow in one direction across the whole system. It means each data entity should have a clearly defined source of truth. Different entities may have different sources of truth.

For example:

```text
Product catalog:
  cloud -> local

Local orders:
  local -> cloud

Device telemetry:
  local -> cloud

Configuration:
  cloud -> local
```

In this model, data flows in both directions overall, but each entity has a single write owner. This greatly reduces synchronization complexity.

## Favor platform-specific synchronization when appropriate

The most efficient and robust data synchronization mechanisms are often designed for specific database platforms, such as PostgreSQL, SQL Server, MySQL, or Oracle. These platform-native capabilities usually provide better performance, operational tooling, transaction semantics, and vendor-backed support.

For example, depending on the database platform and topology, you may consider:

* PostgreSQL logical replication
* SQL Server replication / Always On availability groups
* MySQL replication
* Oracle GoldenGate
* Cloud-provider database replication services

When the application already depends on a specific database platform, platform-native synchronization should be considered first.

## Consider open CDC-based patterns for portability

For more portable, event-driven synchronization, change data capture (CDC) can be used to capture database changes and publish them to a stream or message broker.

A common pattern is:

```text
Relational database -> CDC connector -> event stream -> downstream consumers / local database / cache / analytics store
```

Open-source technologies such as [Debezium](https://debezium.io/) can be used in this pattern, especially when the goal is to capture changes from relational databases and distribute them to other systems.

CDC-based synchronization is often a good fit for one-way replication, event-driven integration, audit trails, and building local read models. However, CDC alone does not solve higher-level business issues such as conflict resolution, schema compatibility, or ownership of writes.

## Be cautious with bidirectional synchronization

Bidirectional synchronization should be used only when there is a clear need and a well-defined conflict-resolution strategy.

Before choosing bidirectional sync, define:

* Which side can create records?
* Which side can update records?
* What happens if both sides update the same record?
* Which timestamp or version wins?
* Are conflicts automatically resolved or manually reviewed?
* How are deletes represented?
* Are deletes physical deletes or tombstones?

In many business applications, the need for bidirectional sync can be reduced by redesigning data ownership boundaries.

## Use explicit data schema versioning

Schema changes are one of the most complicated aspects of data synchronization. A sync solution that works well for stable schemas can break when tables, columns, constraints, or relationships change.

Adaptive App recommends using explicit schema versioning for synchronized data. Each synchronized payload or replicated entity should carry enough version information to allow consumers to interpret it correctly. Schema versioning helps support:

* Backward compatibility
* Forward compatibility
* Rolling upgrades
* Local/cloud version mismatch
* Safe migration during disconnected operation

When possible, schema changes should be additive first. Removing or changing the meaning of fields should be handled through planned migration steps.

## Define reconciliation and recovery behavior

Disconnection is only half of the problem. The more difficult part is often reconnection.

When cloud connectivity is restored, the system should have a clear recovery process:

* How queued local changes are uploaded
* How failed changes are retried
* How duplicate submissions are detected
* How conflicts are reported
* How operators can inspect sync status
* How the system returns from degraded mode to normal mode

Business continuity design should include not only offline operation, but also resynchronization and recovery.
