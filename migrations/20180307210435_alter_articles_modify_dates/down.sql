ALTER TABLE public.articles ALTER COLUMN created_at TYPE TIMESTAMP USING created_at::TIMESTAMP;
ALTER TABLE public.articles ALTER COLUMN updated_at TYPE TIMESTAMP USING updated_at::TIMESTAMP;
ALTER TABLE public.articles ALTER COLUMN updated_at DROP NOT NULL;