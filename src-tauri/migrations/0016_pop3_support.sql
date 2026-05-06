-- POP3 协议支持 + 轮询间隔 + 本地 UIDL 对比
ALTER TABLE email_sources ADD COLUMN protocol TEXT NOT NULL DEFAULT 'imap';
ALTER TABLE email_sources ADD COLUMN poll_interval_seconds INTEGER NOT NULL DEFAULT 60;
ALTER TABLE email_sources ADD COLUMN processed_uidls TEXT NOT NULL DEFAULT '';
