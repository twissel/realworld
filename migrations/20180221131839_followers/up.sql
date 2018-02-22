CREATE TABLE public.followers
(
    id SERIAL PRIMARY KEY,
    user_id INT NOT NULL,
    follower_id INT NOT NULL,
    CONSTRAINT followers_users_id_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT followers_users_f_fk FOREIGN KEY (follower_id) REFERENCES users (id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX followers_user_follower_uindex ON public.followers (user_id, follower_id);