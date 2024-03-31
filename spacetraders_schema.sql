--
-- PostgreSQL database dump
--

-- Dumped from database version 16.2
-- Dumped by pg_dump version 16.2 (Ubuntu 16.2-1.pgdg22.04+1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: public; Type: SCHEMA; Schema: -; Owner: pg_database_owner
--

CREATE SCHEMA public;


ALTER SCHEMA public OWNER TO pg_database_owner;

--
-- Name: SCHEMA public; Type: COMMENT; Schema: -; Owner: pg_database_owner
--

COMMENT ON SCHEMA public IS 'standard public schema';


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: market_transactions; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.market_transactions (
    "timestamp" timestamp with time zone NOT NULL,
    market_symbol text NOT NULL,
    symbol text NOT NULL,
    ship_symbol text NOT NULL,
    type text NOT NULL,
    units integer NOT NULL,
    price_per_unit integer NOT NULL,
    total_price integer NOT NULL
);


ALTER TABLE public.market_transactions OWNER TO postgres;

--
-- Name: market_trades; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.market_trades (
    id bigint NOT NULL,
    "timestamp" timestamp with time zone NOT NULL,
    market_symbol text NOT NULL,
    symbol text NOT NULL,
    trade_volume integer NOT NULL,
    type text NOT NULL,
    supply text NOT NULL,
    activity text,
    purchase_price integer NOT NULL,
    sell_price integer NOT NULL
);


ALTER TABLE public.market_trades OWNER TO postgres;

--
-- Name: general_lookup; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.general_lookup (
    reset_id text NOT NULL,
    key text NOT NULL,
    value json NOT NULL,
    inserted_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);


ALTER TABLE public.general_lookup OWNER TO postgres;

--
-- Name: jumpgate_connections; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.jumpgate_connections (
    reset_id text NOT NULL,
    waypoint_symbol text NOT NULL,
    edges text[] NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);


ALTER TABLE public.jumpgate_connections OWNER TO postgres;

--
-- Name: market_trades_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.market_trades_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.market_trades_id_seq OWNER TO postgres;

--
-- Name: market_trades_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.market_trades_id_seq OWNED BY public.market_trades.id;


--
-- Name: surveys; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.surveys (
    reset_id text NOT NULL,
    uuid uuid NOT NULL,
    survey json NOT NULL,
    asteroid_symbol text NOT NULL,
    inserted_at timestamp with time zone NOT NULL,
    expires_at timestamp with time zone NOT NULL
);


ALTER TABLE public.surveys OWNER TO postgres;

--
-- Name: systems; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.systems (
    id bigint NOT NULL,
    reset_id text NOT NULL,
    symbol text NOT NULL,
    type text NOT NULL,
    x integer NOT NULL,
    y integer NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);


ALTER TABLE public.systems OWNER TO postgres;

--
-- Name: systems_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.systems_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.systems_id_seq OWNER TO postgres;

--
-- Name: systems_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.systems_id_seq OWNED BY public.systems.id;


--
-- Name: waypoint_details; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.waypoint_details (
    id bigint NOT NULL,
    reset_id text NOT NULL,
    waypoint_id bigint NOT NULL,
    is_market boolean NOT NULL,
    is_shipyard boolean NOT NULL,
    is_uncharted boolean NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    is_under_construction boolean NOT NULL
);


ALTER TABLE public.waypoint_details OWNER TO postgres;

--
-- Name: waypoint_details_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.waypoint_details_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.waypoint_details_id_seq OWNER TO postgres;

--
-- Name: waypoint_details_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.waypoint_details_id_seq OWNED BY public.waypoint_details.id;


--
-- Name: waypoints; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.waypoints (
    id bigint NOT NULL,
    reset_id text NOT NULL,
    symbol text NOT NULL,
    system_id bigint NOT NULL,
    type text NOT NULL,
    x integer NOT NULL,
    y integer NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);


ALTER TABLE public.waypoints OWNER TO postgres;

--
-- Name: waypoints_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.waypoints_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.waypoints_id_seq OWNER TO postgres;

--
-- Name: waypoints_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.waypoints_id_seq OWNED BY public.waypoints.id;


--
-- Name: market_trades id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.market_trades ALTER COLUMN id SET DEFAULT nextval('public.market_trades_id_seq'::regclass);


--
-- Name: systems id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.systems ALTER COLUMN id SET DEFAULT nextval('public.systems_id_seq'::regclass);


--
-- Name: waypoint_details id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.waypoint_details ALTER COLUMN id SET DEFAULT nextval('public.waypoint_details_id_seq'::regclass);


--
-- Name: waypoints id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.waypoints ALTER COLUMN id SET DEFAULT nextval('public.waypoints_id_seq'::regclass);


--
-- Name: general_lookup general_lookup_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.general_lookup
    ADD CONSTRAINT general_lookup_pkey PRIMARY KEY (reset_id, key);


--
-- Name: jumpgate_connections jumpgate_connections_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.jumpgate_connections
    ADD CONSTRAINT jumpgate_connections_pkey PRIMARY KEY (reset_id, waypoint_symbol);


--
-- Name: market_trades market_trades_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.market_trades
    ADD CONSTRAINT market_trades_pkey PRIMARY KEY (id, "timestamp");


--
-- Name: market_transactions market_transactions_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.market_transactions
    ADD CONSTRAINT market_transactions_pkey PRIMARY KEY (market_symbol, "timestamp");


--
-- Name: surveys surveys_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.surveys
    ADD CONSTRAINT surveys_pkey PRIMARY KEY (reset_id, uuid);


--
-- Name: systems systems_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.systems
    ADD CONSTRAINT systems_pkey PRIMARY KEY (id);


--
-- Name: waypoint_details waypoint_details_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.waypoint_details
    ADD CONSTRAINT waypoint_details_pkey PRIMARY KEY (id);


--
-- Name: waypoints waypoints_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.waypoints
    ADD CONSTRAINT waypoints_pkey PRIMARY KEY (id);


--
-- Name: market_trades_timestamp_idx; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX market_trades_timestamp_idx ON public.market_trades USING btree ("timestamp" DESC);


--
-- Name: market_transactions_timestamp_idx; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX market_transactions_timestamp_idx ON public.market_transactions USING btree ("timestamp" DESC);


--
-- Name: systems_unique_idx; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX systems_unique_idx ON public.systems USING btree (reset_id, symbol);


--
-- Name: waypoint_details_waypoint_idx; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX waypoint_details_waypoint_idx ON public.waypoint_details USING btree (waypoint_id);


--
-- Name: waypoints_details_idx; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX waypoints_details_idx ON public.waypoint_details USING btree (reset_id);


--
-- Name: waypoints_system_idx; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX waypoints_system_idx ON public.waypoints USING btree (system_id);


--
-- Name: waypoints_unique_idx; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX waypoints_unique_idx ON public.waypoints USING btree (reset_id, symbol);


--
-- Name: market_trades ts_insert_blocker; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER ts_insert_blocker BEFORE INSERT ON public.market_trades FOR EACH ROW EXECUTE FUNCTION _timescaledb_functions.insert_blocker();


--
-- Name: market_transactions ts_insert_blocker; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER ts_insert_blocker BEFORE INSERT ON public.market_transactions FOR EACH ROW EXECUTE FUNCTION _timescaledb_functions.insert_blocker();


--
-- PostgreSQL database dump complete
--

