ALTER TABLE public.users RENAME COLUMN password TO token;
ALTER TABLE public.users ALTER COLUMN token TYPE TEXT USING token::TEXT;