# tero.platform



![Tero mascot](https://images.vexels.com/content/212931/preview/uruguay-tero-hand-drawn-81fe46.png)

## Table of contents
- [What is tero?](#what-is-tero)
- [Architecture](#architecture)
- [Prerequisites for running it locally](#prerequisites-for-running-it-locally)

<br/>

---

<br/>

### What is Tero?

Tero is a platform designed for social gatherings. At its core, it is a social gaming platform, but it is currently being developed into a hub offering much more, such as drink specials, food recommendations, and locations where you can play games in real life.


<br/>

### Architecture

The Tero ecosystem consists of several components, each with a specific role:

- **[tero.platform](https://github.com/Amund-Fremming/tero.platform)** – Written in Rust, this is the core platform that manages all Tero services. It relies on a Rust-based cache called **[gustcache](https://github.com/Amund-Fremming/gustcache)**, which I developed.
- **[tero.session](https://github.com/Amund-Fremming/tero.session)** – A microservice written in C# responsible for managing all game sessions.
- **[tero.app](https://github.com/Amund-Fremming/tero.app)** – The mobile application for iOS, developed using React Native.



<br/>

### Prerequisites for Running Locally

Before running the project locally, make sure you have the following installed:

- [Ngrok](https://ngrok.com/)
- [Rust](https://www.rust-lang.org/)
- [Docker](https://www.docker.com/)
- Authentication setup as described in the [Auth0 documentation](docs/auth0.md)

