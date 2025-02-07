-- Role enum
CREATE TYPE role AS ENUM ('user', 'admin');

-- Sonyflake type
CREATE DOMAIN sonyflake AS VARCHAR(20) NOT NULL;

-- Users table
CREATE TABLE users
(
    id       sonyflake  PRIMARY KEY        NOT NULL UNIQUE,
    email    VARCHAR(320)                  NOT NULL UNIQUE,
    username VARCHAR(32)                   NOT NULL UNIQUE,
    password VARCHAR(128)                  NOT NULL,
    verified BOOLEAN DEFAULT false         NOT NULL,
    role     role    DEFAULT 'user'::role  NOT NULL
);

-- API token table for applications
CREATE TABLE applications
(
    id            sonyflake   PRIMARY KEY   NOT NULL UNIQUE,
    user_id       sonyflake                 NOT NULL,
    name          VARCHAR(16)               NOT NULL,
    last_accessed timestamptz DEFAULT now() NOT NULL,

    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
);

-- A user may not have two duplicate application names
CREATE UNIQUE INDEX applications_name_uindex
    ON applications (user_id, name);

-- Only one verification may exist per user
CREATE TABLE verifications
(
	id          SERIAL  PRIMARY KEY  NOT NULL UNIQUE,
    code        VARCHAR(72)          NOT NULL UNIQUE,
	user_id     sonyflake            NOT NULL UNIQUE,

    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
);

CREATE TABLE files
(
    id             sonyflake  PRIMARY KEY  NOT NULL UNIQUE,
    name           VARCHAR(32)             NOT NULL UNIQUE,
    original_name  VARCHAR(256)            NOT NULL,
    uploader       sonyflake               NOT NULL,
    hash           VARCHAR(64)             NOT NULL,
    uploaded       timestamptz             NOT NULL DEFAULT CURRENT_TIMESTAMP,
    size           BIGINT                  NOT NULL,
    
    -- Application needs to delete the files from the S3 container. This is precautionary for database
    FOREIGN KEY (uploader) REFERENCES users (id) ON DELETE CASCADE
);

-- Two identical files can not exist if owned by the same user
CREATE UNIQUE INDEX files_user_hash_uindex
    ON files (uploader, hash);

-- User registration keys
CREATE TABLE registration_keys
(
    id          sonyflake    NOT NULL PRIMARY KEY,
    iss_user    sonyflake    NOT NULL,
    code        uuid         NOT NULL UNIQUE,
    uses_left   INTEGER      NOT NULL DEFAULT 1,
    expiry_date timestamptz,

    FOREIGN KEY (iss_user) REFERENCES users (id) ON DELETE CASCADE
);