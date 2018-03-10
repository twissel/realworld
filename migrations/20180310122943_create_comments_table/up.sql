CREATE TABLE public.comments
(
    id SERIAL PRIMARY KEY,
    article_id INT NOT NULL,
    user_id INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    body TEXT NOT NULL,
    CONSTRAINT comments_articles_id_fk FOREIGN KEY (article_id) REFERENCES articles (id) ON DELETE CASCADE,
    CONSTRAINT comments_users_id_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
);