CREATE TABLE public.articles
(
    id SERIAL PRIMARY KEY NOT NULL,
    author_id INT NOT NULL,
    slug TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    body TEXT NOT NULL,
    "tagList" TEXT[],
    "createdAt" TIMESTAMP without time zone NOT NULL,
    "updatedAt" TIMESTAMP without time zone,
    CONSTRAINT articles_users_id_fk FOREIGN KEY (author_id) REFERENCES users (id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX articles_slug_uindex ON public.articles (slug);