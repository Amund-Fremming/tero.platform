## Toughts

- add game full / to many to few rounds validation here? Make it owned by the sessions, then implement logic on the session object in tero.session? this will addd some size to the cache but be DDD compliant. Could also just set it in some config, then validate in the hub handlers, or just leave as is no config loaded neccessary
- pagination backend not fe
- maybe go back to feature folder architecture, this will help alot when its alot of games comming in to the platofmr
- move integraitions into config loading
- might be bad to store games as enums f its hard to migrate to expand the enum type in db, then string is better
- Move auth0 id to pseudo user? and rename it to something more generic, Could be nice as nullable to know if the pseudo has a registered user
- Microservice for auth0 to receive events and store in log if it cannot reach main backend. When backend is good an buffer is not empty, batch create
- Move away from auth0, its expencive
- admin page to get tips

- Error handling for client, game full/game does not exist ..
