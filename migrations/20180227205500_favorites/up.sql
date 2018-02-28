CREATE TABLE public.favorites
(
    id SERIAL PRIMARY KEY NOT NULL,
    article_id INT NOT NULL,
    user_id INT NOT NULL,
    CONSTRAINT favorites_articles_id_fk FOREIGN KEY (article_id) REFERENCES articles (id) ON DELETE CASCADE,
    CONSTRAINT favorites_users_id_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
);
CREATE INDEX favorites__article_index ON public.favorites (article_id);
CREATE INDEX favorites__user_index ON public.favorites (user_id);
CREATE UNIQUE INDEX favorites__index_article_user ON public.favorites (article_id, user_id);