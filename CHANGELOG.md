## [Release 0.2.0](https://github.com/aetheric-oss/svc-gis/releases/tag/v0.2.0)

### ✨ Features

- improve debug message output ([`7a5dc4e`](https://github.com/aetheric-oss/svc-gis/commit/7a5dc4e390ff60c58d5cd568442be3cc0e1ce11c))
- update aircraft position interface ([`cefc13a`](https://github.com/aetheric-oss/svc-gis/commit/cefc13a1734e51e389909f06acb9b90ebf8fdb24))
- move table declarations to code ([`0320b0c`](https://github.com/aetheric-oss/svc-gis/commit/0320b0c1c8e0fb4f736de11b87be8733e6ac5c6c))
- move best_path to code, add elevation info ([`5807159`](https://github.com/aetheric-oss/svc-gis/commit/5807159f0fa3430ecdfe1ff6b45b32f564874ff1))
- add postgres-types derive feature to client ([`d7a20b8`](https://github.com/aetheric-oss/svc-gis/commit/d7a20b8dc86fb8c4652c6b871da533eab7efe68b))
- add flight table ([`cc613ca`](https://github.com/aetheric-oss/svc-gis/commit/cc613ca009c64a65a5cc2ebbd64825e0fa828cc5))
- add flight paths check to best path ([`3098359`](https://github.com/aetheric-oss/svc-gis/commit/3098359dac61b533b4c9c5fbb58d2624d59fcbe7))
- add example to test flight avoidance ([`4651f6e`](https://github.com/aetheric-oss/svc-gis/commit/4651f6eb11e3c39fb3e9405c725d08678451e1f3))
- add flight segments table ([`fc55ea6`](https://github.com/aetheric-oss/svc-gis/commit/fc55ea662e817c783a8f3be0795f7310f7e9dd82))
- use flight segments for shortest path ([`722d34e`](https://github.com/aetheric-oss/svc-gis/commit/722d34ecd240ede23c0fad0ac2f9a4af7e66c837))
- add get flights interface ([`a77bebf`](https://github.com/aetheric-oss/svc-gis/commit/a77bebff8f71c59394c513ae1742f1c4acef23a1))
- add session_id to aircraft table ([`920ca34`](https://github.com/aetheric-oss/svc-gis/commit/920ca34bb25f8e9ce86919fd615291f3a2a8d382))
- update get_flights ([`a371e60`](https://github.com/aetheric-oss/svc-gis/commit/a371e601d063f3dee2c7b4128091aa4c194fb72d))
- move flight path addition to grpc call ([`2503556`](https://github.com/aetheric-oss/svc-gis/commit/2503556d268d3496d4fc80fa2ae8c3825a860cef))
- time limit on best path; add intersection check api ([`3cef75e`](https://github.com/aetheric-oss/svc-gis/commit/3cef75e5304a04ada0fea2d0d36bf74eff2677df))

### 🐛 Fixes

- add segment factor and increase waypoint range ([`e3e4e38`](https://github.com/aetheric-oss/svc-gis/commit/e3e4e38f2d7c8ba3c94354d16480fef98587f808))
- refactor segmentize approach ([`7909402`](https://github.com/aetheric-oss/svc-gis/commit/790940230623a30bae9bc1a8484143a40fb3aab4))
- remove pop and process debug calls ([`40bbbaf`](https://github.com/aetheric-oss/svc-gis/commit/40bbbaf5f8e7c11a0399074e8115cb34e3446852))
- update git location to aetheric-oss org ([`ca3a6bc`](https://github.com/aetheric-oss/svc-gis/commit/ca3a6bcb2edcca4bc390108c181c199b2f2c4743))
- postgis no longer in arrow schema ([`5b6be2b`](https://github.com/aetheric-oss/svc-gis/commit/5b6be2bf5215fbfa9d7b69242b443c72f2298d4a))

### 🛠 Maintenance

- terraform provisioned file changes ([`fa12692`](https://github.com/aetheric-oss/svc-gis/commit/fa12692783531f0cfc1fcd3a86318039969593f9))
- update cargo dependencies ([`1924a1d`](https://github.com/aetheric-oss/svc-gis/commit/1924a1dde77f5d32948ab51f22329e3af7b5ec2c))
- bring repo in line with template ([`616630f`](https://github.com/aetheric-oss/svc-gis/commit/616630f4cad29c7e63c68eff73f3392035d86f96))
- switch to redis consumer approach ([`9ddb1e0`](https://github.com/aetheric-oss/svc-gis/commit/9ddb1e0ad246491d004a35911ee1abac36433d5d))
- reviewer comments ([`0846dfb`](https://github.com/aetheric-oss/svc-gis/commit/0846dfb6de39162bef367ca775ff602305a8b0fd))
- reviewer comments ([`208ea9f`](https://github.com/aetheric-oss/svc-gis/commit/208ea9f19d9c58fd9d971792242d17f2a1b0b87e))
- reviewer comments ([`78dc488`](https://github.com/aetheric-oss/svc-gis/commit/78dc488d42d058a363186d6088d28981cfb97245))
- terraform provisioned file changes ([`fdcc427`](https://github.com/aetheric-oss/svc-gis/commit/fdcc427cd36ef27aaee7855bd1b2696760b84779))
- tofu provisioned file changes ([`a48ed05`](https://github.com/aetheric-oss/svc-gis/commit/a48ed05666630f82918789c6b8afde36e0536705))
- cleanup and update unit tests ([`0a59a87`](https://github.com/aetheric-oss/svc-gis/commit/0a59a8744b6d7f5ec77afacbdcb39d7a40eb9cd3))
- r4 final fixes ([`86dfa34`](https://github.com/aetheric-oss/svc-gis/commit/86dfa3487b200388cf54d390674c173a22ab380d))
- reviewer comments 1 ([`4b2afad`](https://github.com/aetheric-oss/svc-gis/commit/4b2afad046bb3b064a6c75b4ae02743dce94336f))

