create table provider_account_fingerprint_observations (
  id uuid primary key,
  provider_account_ref text not null references provider_accounts (id) on delete cascade,
  sent_model text not null,
  response_model text not null,
  observed_at timestamptz not null
);

create index provider_account_fingerprint_observations_recent_idx
  on provider_account_fingerprint_observations (provider_account_ref, observed_at desc);
