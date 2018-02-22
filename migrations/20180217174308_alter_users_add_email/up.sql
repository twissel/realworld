ALTER TABLE public.users ADD email TEXT NOT NULL;
CREATE UNIQUE INDEX users_email_uindex ON public.users (email);