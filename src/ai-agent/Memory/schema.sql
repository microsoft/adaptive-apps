CREATE SCHEMA IF NOT EXISTS advisor_memory;

CREATE TABLE IF NOT EXISTS advisor_memory.facts (
    source_id text NOT NULL,
    agent_id text NOT NULL,
    partition_key text NOT NULL CHECK (partition_key ~ '^[0-9a-f]{64}$'),
    row_key text NOT NULL CHECK (row_key ~ '^[0-9a-f]{32}$'),
    fact_text text NOT NULL CHECK (length(fact_text) BETWEEN 1 AND 4000),
    created_at timestamptz NOT NULL,
    schema_version integer NOT NULL CHECK (schema_version = 1),
    PRIMARY KEY (source_id, agent_id, partition_key, row_key)
);

CREATE TABLE IF NOT EXISTS advisor_memory.sync_status (
    source_id text NOT NULL,
    agent_id text NOT NULL,
    completed_at timestamptz NOT NULL,
    scanned_count bigint NOT NULL,
    PRIMARY KEY (source_id, agent_id)
);
