ALTER TABLE public.users RENAME COLUMN token TO password;
ALTER TABLE public.users ALTER COLUMN password TYPE VARCHAR(255) USING password::VARCHAR(255);