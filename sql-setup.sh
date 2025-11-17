#!/bin/bash

# database setup script
# creates a postgresql database and required tables

set -e

# config
db_name="gameserver_db"
db_user="gameserver"
db_password="gameserverpass"
db_host="localhost"
db_port="5432"

# colors
green='\033[0;32m'
yellow='\033[1;33m'
red='\033[0;31m'
nc='\033[0m'

echo -e "${yellow}starting database setup...${nc}"

# check postgresql
if ! command -v psql &> /dev/null; then
    echo -e "${red}postgresql not installed${nc}"
    echo "ubuntu/debian: sudo apt install postgresql postgresql-contrib"
    echo "macos: brew install postgresql"
    echo "docker: docker run --name postgres -e POSTGRES_PASSWORD=password -p 5432:5432 -d postgres"
    exit 1
fi

run_sql() {
    PGPASSWORD=$db_password psql -h $db_host -p $db_port -U $db_user -d $db_name -c "$1"
}

can_connect() {
    PGPASSWORD=$db_password psql -h $db_host -p $db_port -U $db_user -d $db_name -c "select 1;" >/dev/null 2>&1
}

echo -e "${yellow}testing database connection...${nc}"
if can_connect; then
    echo -e "${green}database connection ok${nc}"
else
    echo -e "${red}cannot connect to database${nc}"
    echo "1. port-forward running: kubectl port-forward svc/gameserver-postgres 5432:5432"
    echo "2. database exists and user has permissions"
    echo "3. connection details correct"
    exit 1
fi

echo -e "${yellow}creating tables...${nc}"

echo -e "${yellow}creating users table...${nc}"
run_sql "
create table if not exists users (
    username varchar primary key,
    password_hash text,
    authcode varchar default '0',
    user_perms text[] not null default '{}',
    created_at timestamptz default now(),
    updated_at timestamptz default now()
);
create index if not exists idx_users_username on users(username);
"
echo -e "${green}users table created${nc}"

echo -e "${yellow}creating nodes table...${nc}"
run_sql "
create table if not exists nodes (
    nodename varchar primary key,
    ip varchar not null,
    nodetype text default 'unknown',
    nodestatus text default 'unknown',
    created_at timestamptz default now(),
    updated_at timestamptz default now()
);
create index if not exists idx_nodes_nodename on nodes(nodename);
"
echo -e "${green}nodes table created${nc}"

echo -e "${yellow}creating servers table...${nc}"
run_sql "
create table if not exists servers (
    servername varchar primary key,
    provider varchar not null,
    providertype varchar not null,
    location varchar not null,
    created_at timestamptz default now(),
    updated_at timestamptz default now()
);
create index if not exists idx_servers_servername on servers(servername);
"
echo -e "${green}servers table created${nc}"

echo -e "${yellow}creating buttons table...${nc}"
run_sql "
create table if not exists buttons (
    name varchar primary key,
    link varchar default '',
    type varchar default 'default',
    created_at timestamptz default now(),
    updated_at timestamptz default now()
);
create index if not exists idx_buttons_name_lower on buttons(lower(name));
"
echo -e "${green}buttons table created${nc}"

echo -e "${yellow}creating settings table...${nc}"
run_sql "
create table if not exists settings (
    id serial primary key,
    toggled_default_buttons boolean not null default false,
    created_at timestamptz default now(),
    updated_at timestamptz default now()
);
create unique index if not exists idx_settings_singleton on settings((id is not null));
"
echo -e "${green}settings table created${nc}"

echo -e "${yellow}inserting default buttons...${nc}"
run_sql "
insert into buttons (name, link, type) values
    ('filebrowser', '', 'default'),
    ('statistics', '', 'default'),
    ('workflows', '', 'default'),
    ('intergrations', '', 'default'),
    ('backups', '', 'default'),
    ('settings', '', 'default')
on conflict (name) do nothing;
"
echo -e "${green}default buttons inserted${nc}"

echo -e "${yellow}inserting default settings...${nc}"
run_sql "
insert into settings (toggled_default_buttons)
select false
where not exists (select 1 from settings);
"
echo -e "${green}default settings inserted${nc}"

echo -e "${yellow}database summary:${nc}"
run_sql "
select schemaname, tablename, hasindexes, hasrules, hastriggers
from pg_tables
where schemaname = 'public'
order by tablename;
"

echo -e "${green}database setup completed${nc}"
echo -e "${yellow}connection details:${nc}"
echo "host: $db_host"
echo "port: $db_port"
echo "database: $db_name"
echo "user: $db_user"
echo ""
echo -e "${yellow}connection string:${nc}"
echo "postgresql://$db_user:$db_password@$db_host:$db_port/$db_name"
