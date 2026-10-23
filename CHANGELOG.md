# [0.38.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.37.2...v0.38.0) (2026-10-22)


### Bug Fixes

* **agent:** keep a session's events when its process exits before it opens ([ed70616](https://github.com/MSpiechowicz/harness-useful-desktop/commit/ed706167e7190ad837997557387c423a93e06d55))
* **chat:** even gaps between composer controls, a gauge for thinking ([7dd2262](https://github.com/MSpiechowicz/harness-useful-desktop/commit/7dd2262409c6ef8746fd363539fabfebecba3188))
* **chat:** thinking uses a sparkles icon, the brain stays for memory ([2850a27](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2850a275d8f19475490448a69f62aebec7f694b9))
* **claude:** a plugin install in an untrusted folder says how to trust it ([7026ab5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/7026ab57829d76350fe1aece2c27442c472b2135))
* **claude:** chat-side fixes from review round 2 ([0385bda](https://github.com/MSpiechowicz/harness-useful-desktop/commit/0385bdad9f43bc889057ab73c930f2eda40f6405))
* **claude:** engine and mapper robustness from review round 1 ([5089033](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5089033c1b5d5ed30cd9c68f37eeaff93ffb9527))
* **claude:** files-side fixes from review round 2 ([5a0a0c2](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5a0a0c27c6adc34ae8bd43749306437e8f405424))
* **claude:** files, CLI and setup hardening from review round 1 ([baa5bad](https://github.com/MSpiechowicz/harness-useful-desktop/commit/baa5badd8a53c446825b950828bf724accb9d842))
* **claude:** project-files approvals only for paths the CLI reads the same way ([5f40728](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5f40728056fa425985a7c11792c1a2901fcce32d))
* **claude:** remember a folder trusted once for the app run ([6f8622c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/6f8622ccbd6f63bcd215d98a14961d8744ced3c2))
* **claude:** safer always-allow, MCP-free helper chat and a folder trust gate ([24ca04d](https://github.com/MSpiechowicz/harness-useful-desktop/commit/24ca04d274dbe104781a119b2a3aeedf6fc702f9))
* **ui:** Claude Code is a real orange in the light theme ([fa59388](https://github.com/MSpiechowicz/harness-useful-desktop/commit/fa59388e0f166b190174844180eee730e6d8c7cc)), closes [#b84a00](https://github.com/MSpiechowicz/harness-useful-desktop/issues/b84a00)
* **ui:** Claude Code's light badge reads orange, not OpenCode's red ([104224f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/104224f29ec55574073424d23bf6244bf615f85e)), closes [#9f4200](https://github.com/MSpiechowicz/harness-useful-desktop/issues/9f4200)


### Features

* **chat:** the harness picker shows a breathing dot and the harness's full name ([3330c61](https://github.com/MSpiechowicz/harness-useful-desktop/commit/3330c615bbc86fe64920ad49893194f04c1d61ea))
* **claude:** approval prompts, questions and approval modes in the Claude chat ([8e897cd](https://github.com/MSpiechowicz/harness-useful-desktop/commit/8e897cd9c62a171d226134c3295a4fe7398eb492))
* **claude:** approval UI for Claude Code and chat preview copy ([df52120](https://github.com/MSpiechowicz/harness-useful-desktop/commit/df521203f3aa88043d2029bfa2d1a9c8d896293c))
* **claude:** Auto approval mode for Claude Code chats and routines ([c4fbaef](https://github.com/MSpiechowicz/harness-useful-desktop/commit/c4fbaefbfdeb1fb34f4360e3156fef5686e3f055))
* **claude:** chat engine core over Claude Code's stream-json mode ([7fa2713](https://github.com/MSpiechowicz/harness-useful-desktop/commit/7fa2713036d69f9e438f41cfc6ffc47629292b75))
* **claude:** Claude Code sessions in the Sessions view ([237fc06](https://github.com/MSpiechowicz/harness-useful-desktop/commit/237fc066ab574c2ee55ae3a9b8493ce95a426290))
* **claude:** fork, clone and switch keep the Harness Skills default model ([0730477](https://github.com/MSpiechowicz/harness-useful-desktop/commit/07304772f825327ef2a4af6e145508ebe37d56d0))
* **claude:** Harness Skills model roles and workflow switches in the Claude config view ([4e07e34](https://github.com/MSpiechowicz/harness-useful-desktop/commit/4e07e34b046e0c8c0497586915f67880af43f913))
* **claude:** name Claude Code in the app copy, readable badge on light themes ([a0aa63c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/a0aa63caf547fb3f5aff7501d5a34e740a20bca9)), closes [#ac3708](https://github.com/MSpiechowicz/harness-useful-desktop/issues/ac3708)
* **claude:** new chats start on the Harness Skills default model, no thinking picker without a model ([c73eddb](https://github.com/MSpiechowicz/harness-useful-desktop/commit/c73eddb674d66547a96cb87867f4b8e315b66d3c))
* **claude:** plugin actions, Harness Skills and MCP registration through the CLI ([9f00105](https://github.com/MSpiechowicz/harness-useful-desktop/commit/9f00105e7eec67860bd542b45e8221277b4b028c))
* **claude:** read-only extensions view for Claude Code ([ec478cf](https://github.com/MSpiechowicz/harness-useful-desktop/commit/ec478cf97f8cb907b243803801767a4085858c20))
* **claude:** redacted config view and deny rules under CLAUDE_CONFIG_DIR ([307e262](https://github.com/MSpiechowicz/harness-useful-desktop/commit/307e2626566740c76be6e68b6f9656cd59db0a00))
* **claude:** register Claude Code as a detected harness with chat off ([e4470fd](https://github.com/MSpiechowicz/harness-useful-desktop/commit/e4470fd5f79a519097adda3607bd3519454d475d))
* **claude:** session operations and Claude Code chat as a preview ([4e79d04](https://github.com/MSpiechowicz/harness-useful-desktop/commit/4e79d0449fa4cda5ac4caa79599157e251da6372))
* **claude:** setup and share copy for CLI-run MCP changes ([19536d1](https://github.com/MSpiechowicz/harness-useful-desktop/commit/19536d1f8c44440d967097371ceab399994aa381))
* **claude:** Share setup for Claude Code marketplaces, plugins and MCP servers ([f7d2d53](https://github.com/MSpiechowicz/harness-useful-desktop/commit/f7d2d538394af4fb42ce745b7a752e42ded84893))
* **claude:** stream-json wire types, event mapper and session stats ([93743b5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/93743b51bce016c837940566f80e83cc3db44d11))
* **claude:** tool ids and the files a Claude Code tool call touched ([f4f0ed7](https://github.com/MSpiechowicz/harness-useful-desktop/commit/f4f0ed70c9200f55ec8663113cfc6863c37ee4b4))
* **i18n:** Brazilian Portuguese as a sixth language ([d9a4de0](https://github.com/MSpiechowicz/harness-useful-desktop/commit/d9a4de0041b79f9e3045bc9bef9e848597992cc6))
* **opencode:** Harness Skills model roles and workflow switches in the OpenCode config view ([eec667b](https://github.com/MSpiechowicz/harness-useful-desktop/commit/eec667be90c551c34fdfab8193a2366a009a96e4))
* **routines:** agent routines name any registered harness ([608632c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/608632c6e85272a156ea29c0c850ff0e54087f97))

## [0.37.2](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.37.1...v0.37.2) (2026-10-22)


### Bug Fixes

* **chat:** hint at an outdated agent when its start times out or breaks the protocol ([a4df37e](https://github.com/MSpiechowicz/harness-useful-desktop/commit/a4df37e88fdb3f6df1fc450a85b6c68af520864d))
* **memory:** show a loading state instead of an empty list while a cleared filter is read ([d06f4e4](https://github.com/MSpiechowicz/harness-useful-desktop/commit/d06f4e4758dfbf370738b3d40ee89ebab59452a6))

## [0.37.1](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.37.0...v0.37.1) (2026-10-22)


### Bug Fixes

* **chat:** sent image thumbnails no longer go to older history messages ([fc9917a](https://github.com/MSpiechowicz/harness-useful-desktop/commit/fc9917a2e8c6f0bdf9b8805a1597c026131e072c))

# [0.37.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.36.0...v0.37.0) (2026-10-21)


### Bug Fixes

* **opencode:** a command right after one that answered waits for its idle ([a9a7bb4](https://github.com/MSpiechowicz/harness-useful-desktop/commit/a9a7bb4be4c05a285cc75ad5825073915ef74c2b))


### Features

* **memory:** save guards for notes and scripts ([#39](https://github.com/MSpiechowicz/harness-useful-desktop/issues/39)) ([c6a9389](https://github.com/MSpiechowicz/harness-useful-desktop/commit/c6a93896567588dc86e3da35cee410adec9955ad))

# [0.36.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.35.3...v0.36.0) (2026-10-21)


### Bug Fixes

* **ci:** green Windows and macOS tests, OpenCode out of the duplication guard ([1fab3f4](https://github.com/MSpiechowicz/harness-useful-desktop/commit/1fab3f43533f8a3b62d3a73bd8cb91dac8b037de))
* **opencode:** adopt subagent sessions only when OpenCode lists them as children ([d18ecf0](https://github.com/MSpiechowicz/harness-useful-desktop/commit/d18ecf09f5cc61cbf355915ae95b41369f3f5bd3))
* **opencode:** an ask for an unprobed subagent never shows before the mode check ([62b8a32](https://github.com/MSpiechowicz/harness-useful-desktop/commit/62b8a32e9ee7e5a82de37b2f9999e1e961994127))
* **opencode:** box the plugin entry so Windows clippy passes ([cc364d4](https://github.com/MSpiechowicz/harness-useful-desktop/commit/cc364d49ef60abd37dea08bbd095fb801a45ae4d))
* **opencode:** guard covers the OpenCode 2 engine, trusted orphan registry, home config in Share checks ([2f792bf](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2f792bfa27ee6824583a808989c60f4087e59c08))
* **opencode:** live shell output and v1-equal tool cards on OpenCode 2 ([b0761df](https://github.com/MSpiechowicz/harness-useful-desktop/commit/b0761df7662132ec0c8c8dfe7e49169605adad10))
* **opencode:** never remove a legacy MCP server the app does not own ([ec63e85](https://github.com/MSpiechowicz/harness-useful-desktop/commit/ec63e8521db98d387aafc03d3342602f7d73baba))
* **opencode:** OpenCode 2 chat recovers asks and subagents after a reconnect ([e8c7cc6](https://github.com/MSpiechowicz/harness-useful-desktop/commit/e8c7cc6e78cda12eaa43c84163c3976148933701))
* **opencode:** OpenCode 2 install hint, project-config switches and safer orphan sweep ([5af3e18](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5af3e18d6960b4f1827ad10ee3d37c43c2bd0654))
* **opencode:** OpenCode 2 mode chats re-check reloaded agents, heartbeats keep the stream ([465d631](https://github.com/MSpiechowicz/harness-useful-desktop/commit/465d631c3169ced660cf2aaddcd1bc619457b427))
* **opencode:** re-check the mode before caught-up asks, mask a trailing password piece ([8f55696](https://github.com/MSpiechowicz/harness-useful-desktop/commit/8f556964d202f566ad54f606c232489f65dbc204))
* **process:** a failing ps keeps an app alive, owner-only orphan files, guard skips comments ([a6e3511](https://github.com/MSpiechowicz/harness-useful-desktop/commit/a6e3511afbb35efb2c341e2a6724b53e331323f6))
* **process:** keep an orphan record that cannot be read right now ([055ef31](https://github.com/MSpiechowicz/harness-useful-desktop/commit/055ef31052b122670e2711679d382b1347451381))
* **process:** kill chat servers a crashed app left behind ([81fcfbf](https://github.com/MSpiechowicz/harness-useful-desktop/commit/81fcfbf776beea3d6f7f95652880d852aa51d3ec))


### Features

* **opencode:** approvals, approval modes, questions and helper chat on OpenCode 2 ([0bfdd0a](https://github.com/MSpiechowicz/harness-useful-desktop/commit/0bfdd0a5b0c2fb624a3e0ce201977134ac76dc3a))
* **opencode:** detect OpenCode 2 and route by generation ([9d0c979](https://github.com/MSpiechowicz/harness-useful-desktop/commit/9d0c9795f8e9d433e86d25f279c530fc049a0ed5))
* **opencode:** map OpenCode 2 events to chat events ([31f0d59](https://github.com/MSpiechowicz/harness-useful-desktop/commit/31f0d59aa32884b55b7b1a3f03986e6cd9002d43))
* **opencode:** OpenCode 2 chat engine core ([86d3c17](https://github.com/MSpiechowicz/harness-useful-desktop/commit/86d3c174c63e55bf4d30278b711e2d29fda8cf4c))
* **opencode:** OpenCode 2 routes, wire types and events ([12c456f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/12c456f4f1f00287482e85d13956a68cea5717bd))
* **opencode:** plugin and MCP writes for OpenCode 2 ([88aee95](https://github.com/MSpiechowicz/harness-useful-desktop/commit/88aee95b002b2b0958bcf4a441b1114d72a5c964))
* **opencode:** read OpenCode 2 config and extensions ([d8285aa](https://github.com/MSpiechowicz/harness-useful-desktop/commit/d8285aaf38ad7791b6e610980fbb4a73d14b5883))
* **opencode:** session operations for OpenCode 2 chat ([248aec4](https://github.com/MSpiechowicz/harness-useful-desktop/commit/248aec4c9667202dfd00c38c450ce6b96b1ed659))
* **opencode:** sessions for OpenCode 2 ([00afe63](https://github.com/MSpiechowicz/harness-useful-desktop/commit/00afe6340e9d9f1615f24f671f212f3d2995f435))
* **opencode:** skills in the slash menu, background commands, summed stats on OpenCode 2 ([87a02c6](https://github.com/MSpiechowicz/harness-useful-desktop/commit/87a02c6074ec141468ba615b1e28269e58e8411f))
* **ui:** OpenCode 2 aware MCP snippets and import preview ([848d80e](https://github.com/MSpiechowicz/harness-useful-desktop/commit/848d80e95eed583e8194be9b06c7a8799afb45ac))

## [0.35.3](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.35.2...v0.35.3) (2026-10-21)


### Bug Fixes

* **updates:** recheck for new releases while the app is open ([16d46fc](https://github.com/MSpiechowicz/harness-useful-desktop/commit/16d46fc66cff117f3034a9c8dbcf10c78f926eef))

## [0.35.2](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.35.1...v0.35.2) (2026-10-21)


### Bug Fixes

* **chat:** uppercase branch names are no longer clipped in the composer footer ([9256ca5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/9256ca506649be56fda4596a6496331a2bf0225c))

## [0.35.1](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.35.0...v0.35.1) (2026-10-21)


### Bug Fixes

* **hooks:** let the memory-save hook save every note that is useful ([abb9fdc](https://github.com/MSpiechowicz/harness-useful-desktop/commit/abb9fdca003cb79078093a74b05820a49cd02340))

# [0.35.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.34.0...v0.35.0) (2026-10-21)


### Features

* **hooks:** add branch-sync family hook and make the memory hooks selective ([88badd6](https://github.com/MSpiechowicz/harness-useful-desktop/commit/88badd670f59e730f1da0a00806da90fe025d431))

# [0.34.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.33.0...v0.34.0) (2026-10-21)


### Bug Fixes

* **deps:** update markdown-it to 15.0.3 for GHSA-vfgh-jcmw-g37q ([141b88d](https://github.com/MSpiechowicz/harness-useful-desktop/commit/141b88dbbd3904eb53449e372daad6a079f9ea2f)), closes [GHSA-vf#jcmw-g37q](https://github.com/GHSA-vf/issues/jcmw-g37q)
* **deps:** update rustls-webpki and webpki-root-certs for RUSTSEC-2026-0382 and -0383 ([77aa832](https://github.com/MSpiechowicz/harness-useful-desktop/commit/77aa832a95181cf59b48bc0dd400ca8903ba08af))
* **extensions:** treat Windows paths as local OpenCode plugin files when removing ([8df1871](https://github.com/MSpiechowicz/harness-useful-desktop/commit/8df187164636ea8f8f05680f1706a8719d8c549a))
* **hooks,routines:** give the name column room next to Scope and align the menu search ([9cad6bd](https://github.com/MSpiechowicz/harness-useful-desktop/commit/9cad6bd212b26c6adb1d8bdffa740f772f23d606)), closes [#37](https://github.com/MSpiechowicz/harness-useful-desktop/issues/37)
* **hooks:** check a team hook's repository with the server before copying it to a project ([9098ee4](https://github.com/MSpiechowicz/harness-useful-desktop/commit/9098ee42b902ada9fe3d61f6ac7b445896d128f2)), closes [#37](https://github.com/MSpiechowicz/harness-useful-desktop/issues/37)
* **memory,scope:** give the badges in table rows the size of the status badges ([886e512](https://github.com/MSpiechowicz/harness-useful-desktop/commit/886e512cc08073c960e11539e13f134c5f748f65))
* **memory:** let Delete all count the Kept in filter only where it applies ([1fb684c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/1fb684c2e7932724c15322a68136c40b5df9f948))
* **memory:** show the Kept in filter whenever Harness Memory can filter by place ([8867b55](https://github.com/MSpiechowicz/harness-useful-desktop/commit/8867b558aa8faff634b817e1b71616aba8ff9bb3))
* **onboarding:** hide the sessions tile when the count was cut off before counting anything ([ac4d48b](https://github.com/MSpiechowicz/harness-useful-desktop/commit/ac4d48bd1eb89a4e2578cad4b14f2fc66dbf8795))
* **onboarding:** one control height in the harness picker, no Not found for coming-soon harnesses ([fce6330](https://github.com/MSpiechowicz/harness-useful-desktop/commit/fce63306599699c7af82556e0d1b069f7731a4ef))
* **onboarding:** use the large badge in the harness intro and picker, like the Harness family cards ([5c5993f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5c5993f58501dfd16cca2d984f77a2e63c79a9dd))
* **onboarding:** widen the harness list so names and the Daily driver chip fit ([fa87c2f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/fa87c2ff6e9526002ca197fbaca5ab69404a6ec7))
* **scope:** keep Replace, Delete all and the open row true to the project chosen ([635d51c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/635d51c64bec3a30b08c3b2f066fed78f376a073)), closes [#37](https://github.com/MSpiechowicz/harness-useful-desktop/issues/37)
* **scope:** keep the filters chosen under an open panel when the view opens again ([0ec4580](https://github.com/MSpiechowicz/harness-useful-desktop/commit/0ec4580300b830bde101755a5c64a868a9e57875)), closes [#37](https://github.com/MSpiechowicz/harness-useful-desktop/issues/37)
* **setup:** register OpenCode MCP servers only after the Harness Skills install settles ([b82b2e3](https://github.com/MSpiechowicz/harness-useful-desktop/commit/b82b2e3334b9984afe7550753b57def786b34ad9)), closes [#25](https://github.com/MSpiechowicz/harness-useful-desktop/issues/25)
* **share:** ask OpenCode for its merged config only when importing, never while reviewing ([7722907](https://github.com/MSpiechowicz/harness-useful-desktop/commit/7722907fcf6c9d0b352b387e1c57710351451d59)), closes [#24](https://github.com/MSpiechowicz/harness-useful-desktop/issues/24)
* **share:** check OpenCode's merged config before importing an MCP server ([230f487](https://github.com/MSpiechowicz/harness-useful-desktop/commit/230f487ecf9172e26de202d2a79fc0ea86f1b225)), closes [#24](https://github.com/MSpiechowicz/harness-useful-desktop/issues/24)
* **share:** fail closed on unreadable OpenCode config layers and reserve the Harness Skills server ([c17ae9e](https://github.com/MSpiechowicz/harness-useful-desktop/commit/c17ae9e2a3eb61783986343e7c63ab17d9100dbe)), closes [#23](https://github.com/MSpiechowicz/harness-useful-desktop/issues/23) [#24](https://github.com/MSpiechowicz/harness-useful-desktop/issues/24) [#25](https://github.com/MSpiechowicz/harness-useful-desktop/issues/25)
* **share:** keep the probe's temp file unshared on Windows and document that it runs OpenCode plugins ([53a175a](https://github.com/MSpiechowicz/harness-useful-desktop/commit/53a175aa1ebdfe8298a2eefc2b70140879e26df9))
* **share:** name unreadable config layers, fail only unwritten servers on a failed OpenCode check, and give the Node message a code ([bc6fc06](https://github.com/MSpiechowicz/harness-useful-desktop/commit/bc6fc06415aa513ee779394c7a70fdf7848353d9)), closes [#24](https://github.com/MSpiechowicz/harness-useful-desktop/issues/24) [#25](https://github.com/MSpiechowicz/harness-useful-desktop/issues/25)
* **ui:** one 36px button height and one badge size in onboarding, button labels lifted to sit optically centred ([f4937da](https://github.com/MSpiechowicz/harness-useful-desktop/commit/f4937dac4f1bf35cc0d04092bc8988fa7db7db30))


### Features

* **extensions:** install and remove OpenCode plugins (backend) ([afb28f5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/afb28f5dc697f32e7a319a65541fb295c8dd938a)), closes [#23](https://github.com/MSpiechowicz/harness-useful-desktop/issues/23)
* **extensions:** install and remove OpenCode plugins from the Extensions view ([d63cf73](https://github.com/MSpiechowicz/harness-useful-desktop/commit/d63cf73b39b6ab74ea346148963d10dc1f7f8aea)), closes [#23](https://github.com/MSpiechowicz/harness-useful-desktop/issues/23)
* **hooks,routines:** one table of every project with a Scope filter, paging and an Applies to dropdown ([0d4857c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/0d4857cc7ede8890cf61f9ee698e6258d2f2e28b)), closes [#37](https://github.com/MSpiechowicz/harness-useful-desktop/issues/37)
* **memory:** add a Kept in filter to the notes and scripts tables ([db18f53](https://github.com/MSpiechowicz/harness-useful-desktop/commit/db18f53cbb71402ae5a81cbff19fe9e2e015689e)), closes [#37](https://github.com/MSpiechowicz/harness-useful-desktop/issues/37)
* **memory:** filter notes and scripts by where they are kept ([ca096a1](https://github.com/MSpiechowicz/harness-useful-desktop/commit/ca096a10d3cac2f8f89f91a72cf4134a1b3ba8a3))
* **memory:** one Scope filter and an Applies to dropdown for notes and scripts ([1b6e26f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/1b6e26fdcab759db0b848fb403c6e407faca9fe1)), closes [#37](https://github.com/MSpiechowicz/harness-useful-desktop/issues/37)
* **memory:** show the Scope of notes and scripts like hooks and routines ([8057781](https://github.com/MSpiechowicz/harness-useful-desktop/commit/8057781066d5f3bf7eb7fb83fc3aa447ae1c43db)), closes [#37](https://github.com/MSpiechowicz/harness-useful-desktop/issues/37)
* **onboarding:** count each harness's sessions across all projects for the picker ([cfebec7](https://github.com/MSpiechowicz/harness-useful-desktop/commit/cfebec77996c3ab7f1710f28a29174ab998abc22))
* **onboarding:** introduce the harnesses first, then pick the daily driver from a list ([5f8017f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5f8017fdcb768d13e8b62c4483c5317ba30b4cb1))
* **onboarding:** keep installer output in the Harness family card's Details ([10d65e1](https://github.com/MSpiechowicz/harness-useful-desktop/commit/10d65e167919cc24c49a453ca095a9dc5f365ad6))
* **onboarding:** one number per machine-facts tile, real labels while loading, facts prefetched on the intro ([c144448](https://github.com/MSpiechowicz/harness-useful-desktop/commit/c144448269f069113077a2ecc23d1dab1a401931))
* **onboarding:** show what is on this machine for an installed harness in the picker ([4391716](https://github.com/MSpiechowicz/harness-useful-desktop/commit/439171692f34615e878f5c3158c42397d9657bbc))
* **scope:** list the scope of every added project and all hooks and routines at once (backend) ([b57b920](https://github.com/MSpiechowicz/harness-useful-desktop/commit/b57b920fcd6a21b78285bdb0a28fe66ce595523a)), closes [#37](https://github.com/MSpiechowicz/harness-useful-desktop/issues/37)
* **scope:** shared Scope helpers and view state for the hooks and routines tables ([49dd67f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/49dd67f28cdd8485135fca3119a756cb0b61c84b)), closes [#37](https://github.com/MSpiechowicz/harness-useful-desktop/issues/37)
* **setup:** install Harness Skills for OpenCode from Recommended setup (backend) ([0fd374f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/0fd374f2a35537c83ea7e984cc3e45a20a927a3a)), closes [#25](https://github.com/MSpiechowicz/harness-useful-desktop/issues/25)
* **setup:** offer Harness Skills for OpenCode in onboarding (UI) ([b25dba0](https://github.com/MSpiechowicz/harness-useful-desktop/commit/b25dba06d39329927ca5a7573c16fbb4796b96ac)), closes [#25](https://github.com/MSpiechowicz/harness-useful-desktop/issues/25)
* **share:** export and import OpenCode MCP servers and plugins (backend) ([a2e45d6](https://github.com/MSpiechowicz/harness-useful-desktop/commit/a2e45d629ad47d09348a97393068ff36d74ee7de)), closes [#24](https://github.com/MSpiechowicz/harness-useful-desktop/issues/24)
* **share:** note that new OpenCode MCP servers are checked when you import ([02b1380](https://github.com/MSpiechowicz/harness-useful-desktop/commit/02b13806e4d383d9e1fcd3dbac36d54283fbcaf8)), closes [#24](https://github.com/MSpiechowicz/harness-useful-desktop/issues/24)
* **share:** show OpenCode MCP servers, plugins and manual edits in Share setup (UI) ([96afc20](https://github.com/MSpiechowicz/harness-useful-desktop/commit/96afc203c34863dee75876b43e5f4205932d34f5)), closes [#24](https://github.com/MSpiechowicz/harness-useful-desktop/issues/24)
* **share:** show unreadable config files and the localised Node.js message ([71462be](https://github.com/MSpiechowicz/harness-useful-desktop/commit/71462bec4c0242162deff9acbb5e69930bbcb3ff)), closes [#24](https://github.com/MSpiechowicz/harness-useful-desktop/issues/24) [#25](https://github.com/MSpiechowicz/harness-useful-desktop/issues/25)

# [0.33.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.32.0...v0.33.0) (2026-10-20)


### Bug Fixes

* **opencode:** accept approval modes when OpenCode reorders its skill rules ([eed4003](https://github.com/MSpiechowicz/harness-useful-desktop/commit/eed4003479c8f45883e54ea6d5071e03adc3b43f))
* **opencode:** bind approval cards to their tool call and mark questions ([b4edcae](https://github.com/MSpiechowicz/harness-useful-desktop/commit/b4edcae0e318c0b502acf06556d161992438bb74))
* **opencode:** deliver approval modes as the chat server's config ([74ec28b](https://github.com/MSpiechowicz/harness-useful-desktop/commit/74ec28bc39e8c429493f2779ff556aaf78095628))
* **opencode:** neutralise stored approval rules and harden the engine ([346daf1](https://github.com/MSpiechowicz/harness-useful-desktop/commit/346daf140898b452fc93550688b78dac59ec4543))
* **opencode:** probe the session's own folder and refuse untrusted agent lists ([5b9c11f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5b9c11f94813c8fd9dc1d155e727950ab0fc3e93))
* **opencode:** show the default model, the switch at once, and no agent picker ([1aab01c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/1aab01cef559c566413d32fafd989aba78b7f212))


### Features

* **model:** agent picker and preview capability in the chat contract ([2f700df](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2f700df12f220e693e17a5e45ae55dac1dbb66e5))
* **opencode:** approvals, questions and per-chat approval modes ([cb3354e](https://github.com/MSpiechowicz/harness-useful-desktop/commit/cb3354e5f76b13e08c6ce9de8e2f0d1e8273dfec))
* **opencode:** enable chat for the OpenCode harness ([7d5f2a7](https://github.com/MSpiechowicz/harness-useful-desktop/commit/7d5f2a744badac155c940298afbce5579e45f737))
* **opencode:** loopback HTTP and SSE client ([5cb2a4c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5cb2a4cb60ad0ee1e152079b7815e7da218477e5))
* **opencode:** native chat engine over opencode serve ([f5062c4](https://github.com/MSpiechowicz/harness-useful-desktop/commit/f5062c4078e8487cdc84a8de758964f31895af55))
* **opencode:** session actions, export, stats and !commands ([4ba6000](https://github.com/MSpiechowicz/harness-useful-desktop/commit/4ba60006cfce486850e09bc92566233c5a4c16cf))
* **opencode:** steering, queue, abort, slash commands and history ([5837d60](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5837d60ce75eef29836d3d836e5e398b8322748b))
* **routines:** OpenCode agent routines and hooks ([89201e8](https://github.com/MSpiechowicz/harness-useful-desktop/commit/89201e8ce80812e8f676a53d826d3c5e93975574))
* **ui:** OpenCode chat with agent picker and Preview badge ([406007a](https://github.com/MSpiechowicz/harness-useful-desktop/commit/406007ad03262ebce5de942739012f0933e56dea))

# [0.32.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.31.0...v0.32.0) (2026-10-20)


### Bug Fixes

* **chat:** match the bottom padding of a system notice card to its sides when it shows a details box ([82969fe](https://github.com/MSpiechowicz/harness-useful-desktop/commit/82969fe6adbd1925ebf66a12ef3cdee36f4fd0bc))


### Features

* **hooks:** add the Check Useful Skills first family hook so agents read installed Useful Skills ([#30](https://github.com/MSpiechowicz/harness-useful-desktop/issues/30)) ([92a96cb](https://github.com/MSpiechowicz/harness-useful-desktop/commit/92a96cbbfe8741ce91e8ee78911e5f354a9d97fe))

# [0.31.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.30.0...v0.31.0) (2026-10-20)


### Bug Fixes

* **chat:** drop the purple line from thinking text and balance row padding ([2ff992e](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2ff992edd8fe1647119aa90fc6afd5610b9ce4eb)), closes [#31](https://github.com/MSpiechowicz/harness-useful-desktop/issues/31)
* **chat:** list running subagents on a running wait card and hide empty args ([#33](https://github.com/MSpiechowicz/harness-useful-desktop/issues/33)) ([9ce84ab](https://github.com/MSpiechowicz/harness-useful-desktop/commit/9ce84ab3cb847cdd6697d47746fbf3a70ebb6fe1))
* **chat:** name the chat again after /new starts a new session ([#35](https://github.com/MSpiechowicz/harness-useful-desktop/issues/35)) ([93e66c5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/93e66c5746eab3e39c9443c832ce5e0694307336))
* **chat:** unwrap system notices whose tag has attributes ([b7102d8](https://github.com/MSpiechowicz/harness-useful-desktop/commit/b7102d8cb1ee2ee3b6cf874f9c79936fa90a2321))
* **chat:** use the chat corner and card border on the queued messages list ([a9726d9](https://github.com/MSpiechowicz/harness-useful-desktop/commit/a9726d9e7941b3c1b6f16945a5de52ad21f5ac80)), closes [#34](https://github.com/MSpiechowicz/harness-useful-desktop/issues/34)
* **ui:** keep the spinner ring still and turn only its arc so the running pill no longer wobbles sideways ([#36](https://github.com/MSpiechowicz/harness-useful-desktop/issues/36)) ([70b2e4b](https://github.com/MSpiechowicz/harness-useful-desktop/commit/70b2e4b139633872b0709b7dcee033d0d6d0fc78))
* **updates:** keep the update bar full while installing instead of dropping to a sweep mid-fill ([#28](https://github.com/MSpiechowicz/harness-useful-desktop/issues/28)) ([f2a389d](https://github.com/MSpiechowicz/harness-useful-desktop/commit/f2a389d695ac57a965f99d765c24d8dfa285bf99))


### Features

* **chat:** pin running subagents above the composer ([7f64a15](https://github.com/MSpiechowicz/harness-useful-desktop/commit/7f64a1568022dabe8b96a29b690396dc855ea99d))
* **chat:** readable tool names in tool cards and approvals ([0fb776d](https://github.com/MSpiechowicz/harness-useful-desktop/commit/0fb776d3b70b2d55693311589b7c3a2895acdfe7))

# [0.30.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.29.0...v0.30.0) (2026-10-20)


### Features

* **memory:** percent-encode MongoDB and DocumentDB credentials in the team database URL ([07cb691](https://github.com/MSpiechowicz/harness-useful-desktop/commit/07cb691bb968eb990f60c1dbd72d93985e4dd946))
* **memory:** Repository filter for Notes and Scripts, notice formatting in chat ([ad9a792](https://github.com/MSpiechowicz/harness-useful-desktop/commit/ad9a79296a707434361daea923fb53fc00acabf4))

# [0.29.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.28.0...v0.29.0) (2026-10-19)


### Features

* **memory:** remote + this computer for notes and scripts, copy local to remote ([bc23b4a](https://github.com/MSpiechowicz/harness-useful-desktop/commit/bc23b4a0edc0d66629f2fc8b6fafa14b22f84777))

# [0.28.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.27.1...v0.28.0) (2026-10-19)


### Bug Fixes

* **ci:** Windows clippy and the duplicate test setup ([5588c67](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5588c67385fdf481a2c9b2b8db1c2c0f8baae01e))
* **memory:** say to update Harness Memory when it cannot save from the app ([d032a3e](https://github.com/MSpiechowicz/harness-useful-desktop/commit/d032a3ea96dffd16a340f3ef6cb0d8ae69cd8c4a))


### Features

* **memory:** Notes and Scripts views with add, edit and notes import ([0d34504](https://github.com/MSpiechowicz/harness-useful-desktop/commit/0d34504c0ae64772ea8db939ac6a132ddf4422f1))
* **settings:** Amazon DocumentDB as a team memory database ([bdd54e7](https://github.com/MSpiechowicz/harness-useful-desktop/commit/bdd54e74edaa789100a904c98ed4a4fff2b4be64))

## [0.27.1](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.27.0...v0.27.1) (2026-10-19)


### Bug Fixes

* **ui:** larger status badges on Ecosystem cards ([684c0ea](https://github.com/MSpiechowicz/harness-useful-desktop/commit/684c0ea04b121313fee4b0c2c7c36d27fccee78f))

# [0.27.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.26.0...v0.27.0) (2026-10-19)


### Bug Fixes

* **test:** pin the clock in the routines next-run sort test ([897a360](https://github.com/MSpiechowicz/harness-useful-desktop/commit/897a360719e454bdde2beb53185ed096a4448b43))


### Features

* **ui:** sortable columns and larger status badges in Hooks and Routines ([58d7661](https://github.com/MSpiechowicz/harness-useful-desktop/commit/58d7661d0e3506bdb871cd802b05db2d01d20819))

# [0.26.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.25.0...v0.26.0) (2026-10-19)


### Bug Fixes

* **chat:** a late agent event no longer fails without a frame loop ([6098c2f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/6098c2f1ff2c5d9ab4b3550a44fe8b98363851d6))
* **ci:** pass the Windows and macOS routine and fixture tests ([d3395d6](https://github.com/MSpiechowicz/harness-useful-desktop/commit/d3395d6032ee09aedfd5812e7d37979b93bd9114))
* **hooks:** agents only read, edit and remove hooks of their own project ([8f4145f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/8f4145fb06f703e7bf9dc0727486c52f826659e0))
* **process:** stopping a command on Windows ends what it started too ([99d6b64](https://github.com/MSpiechowicz/harness-useful-desktop/commit/99d6b64fcae1499a6293a40c2718dcd721c46100))


### Features

* **hooks:** keep Harness family hooks current with automatic updates ([71c2825](https://github.com/MSpiechowicz/harness-useful-desktop/commit/71c2825aeaf0575adbb2aec4a0be7a6417c35498))
* **routines:** run tasks on a schedule and share them with the team ([5290178](https://github.com/MSpiechowicz/harness-useful-desktop/commit/52901789edf2691bf23568e399d9e735400aeccf)), closes [#14](https://github.com/MSpiechowicz/harness-useful-desktop/issues/14)

# [0.25.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.24.0...v0.25.0) (2026-10-18)


### Features

* **hooks:** name Harness Memory's renamed note tools ([675fe25](https://github.com/MSpiechowicz/harness-useful-desktop/commit/675fe258b26fde334fb9dbc37bb04227fe27b470))

# [0.24.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.23.1...v0.24.0) (2026-10-18)


### Bug Fixes

* **test:** give the Windows test environment an APPDATA folder ([4e67ef2](https://github.com/MSpiechowicz/harness-useful-desktop/commit/4e67ef2953bc124abc51def4f1a4910e6d2cab95))
* **test:** let the slow prompt-submit hook test pass on a slow runner ([41a1c54](https://github.com/MSpiechowicz/harness-useful-desktop/commit/41a1c54b774071999a67a6666ca9b08cb365b1a1))


### Features

* **hooks:** share hooks with the team through the Harness Memory team database ([8e30cf0](https://github.com/MSpiechowicz/harness-useful-desktop/commit/8e30cf0cf526d1bc8168d421d336ae78c7a87771)), closes [#26](https://github.com/MSpiechowicz/harness-useful-desktop/issues/26)

## [0.23.1](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.23.0...v0.23.1) (2026-10-18)


### Bug Fixes

* **linux:** children of a relaunched AppImage no longer load the old image's libraries ([047d9c5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/047d9c5568171cff945912c5f15d13fe076d5935))
* **ui:** equal space above and below the top bar controls ([3f6303c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/3f6303c05f010db73865c509c1036d066d351835))

# [0.23.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.22.1...v0.23.0) (2026-10-18)


### Features

* **ui:** one workspace layout, live data without refresh buttons, a steady top bar ([029cecc](https://github.com/MSpiechowicz/harness-useful-desktop/commit/029cecc359a11901d594c184e3cdd19748e27454))

## [0.22.1](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.22.0...v0.22.1) (2026-10-17)


### Bug Fixes

* **hooks:** harden what agents can put into hook commands and names ([fd986f3](https://github.com/MSpiechowicz/harness-useful-desktop/commit/fd986f34b8e203f3579bfc1d8870e592d7b254dd))
* **hooks:** line up the scope, project and count in the group headers ([8248aaf](https://github.com/MSpiechowicz/harness-useful-desktop/commit/8248aaff7c90f80cf873cb3ac661c56c24a900ae))

# [0.22.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.21.0...v0.22.0) (2026-10-17)


### Bug Fixes

* **ci:** pass the Windows and macOS test runs ([1302d40](https://github.com/MSpiechowicz/harness-useful-desktop/commit/1302d4022f179c242e6f2362e2e3debf2917ebbe))
* **memory:** count script groups apart and offer the update only when there is one ([920b4d0](https://github.com/MSpiechowicz/harness-useful-desktop/commit/920b4d0c159b2b547866db2f62f60abaee5a1e0c))


### Features

* **hooks:** run hooks in desktop chats and let agents manage them ([e489a11](https://github.com/MSpiechowicz/harness-useful-desktop/commit/e489a1161fbb3c5457bd2cf372bdebca2e00755e)), closes [#16](https://github.com/MSpiechowicz/harness-useful-desktop/issues/16)

# [0.21.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.20.0...v0.21.0) (2026-10-17)


### Bug Fixes

* **ci:** accept the new zbus unmaintained advisories in the dependency audit ([0a4d0be](https://github.com/MSpiechowicz/harness-useful-desktop/commit/0a4d0be6cad72ac5c263fa0aec928645bd49498a))


### Features

* **memory:** Memory view with notes and scripts, and script approvals ([2ae6509](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2ae6509c1db9a5a3f7b91d1ecfafa5a046df8774))

# [0.20.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.19.0...v0.20.0) (2026-10-17)


### Bug Fixes

* **ci:** pass the duplication guard and the macOS and Windows test runs ([c5e62b1](https://github.com/MSpiechowicz/harness-useful-desktop/commit/c5e62b1399539b30e113f74b5b6e187ae619a0c0))


### Features

* **extensions:** switch harness in Config and Extensions, and manage PI packages ([732143e](https://github.com/MSpiechowicz/harness-useful-desktop/commit/732143e3e66bf40802098b8a0f175f18e554fcc4))
* **opencode:** detect OpenCode and show its config, extensions and sessions ([7dab4c2](https://github.com/MSpiechowicz/harness-useful-desktop/commit/7dab4c299b42949400dba002a119db27e7ac7c94)), closes [#19](https://github.com/MSpiechowicz/harness-useful-desktop/issues/19)

# [0.19.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.18.0...v0.19.0) (2026-10-16)


### Features

* **ecosystem:** keep installed Harness family members up to date automatically ([f48d30c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/f48d30c797e00879d4659506397f174ce60fc304))

# [0.19.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.18.0...v0.19.0) (2026-10-16)


### Features

* **ecosystem:** keep installed Harness family members up to date automatically ([f48d30c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/f48d30c797e00879d4659506397f174ce60fc304))

# [0.18.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.17.0...v0.18.0) (2026-10-16)


### Features

* **memory:** Memory settings with remote (team) memory ([6e56ba7](https://github.com/MSpiechowicz/harness-useful-desktop/commit/6e56ba70a0df85f3cdfed824bacbcc4aa1a8745d)), closes [#8](https://github.com/MSpiechowicz/harness-useful-desktop/issues/8)

# [0.17.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.16.2...v0.17.0) (2026-10-16)


### Bug Fixes

* **memory:** build home paths from components so Windows paths use one separator ([3267d18](https://github.com/MSpiechowicz/harness-useful-desktop/commit/3267d188f1d3036c634d6a016c69c0c20b2e3235))


### Features

* **memory:** install Harness Memory as a family member instead of bundling it ([089c470](https://github.com/MSpiechowicz/harness-useful-desktop/commit/089c470eef3379b622d274a4c55947b5f551fbdd)), closes [#8](https://github.com/MSpiechowicz/harness-useful-desktop/issues/8)

## [0.16.2](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.16.1...v0.16.2) (2026-10-16)


### Bug Fixes

* **ui:** cap config and extensions at the page width, keep the ecosystem scrollbar gutter stable ([81e33d3](https://github.com/MSpiechowicz/harness-useful-desktop/commit/81e33d32faff3864a5dcffb0e1b753d23993efc2))

## [0.16.1](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.16.0...v0.16.1) (2026-10-16)


### Bug Fixes

* **chat:** stop clipping the harness name, centre messages against the composer, render system messages as markdown ([9f0622b](https://github.com/MSpiechowicz/harness-useful-desktop/commit/9f0622b4f5fc914e2254a0eef8fa80964ca8c24d))


### Performance Improvements

* **chat:** restore last run's chats in parallel and show them as starting tabs ([824fed2](https://github.com/MSpiechowicz/harness-useful-desktop/commit/824fed2af32fb3d02f001524834932437ef65080))

# [0.16.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.15.0...v0.16.0) (2026-10-16)


### Bug Fixes

* **chat:** open localhost links, hide model-only harness messages, align config tree summaries ([2aa94f6](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2aa94f6757ce1c8e5c9789cd2048ccf00c951de5))
* **memory:** make the memory tests and path display portable to macOS and Windows ([745f6bc](https://github.com/MSpiechowicz/harness-useful-desktop/commit/745f6bc84477ea53bc192578a21059105867e2cb))


### Features

* **memory:** project memory for every harness through a bundled harness-memory MCP server ([56806ce](https://github.com/MSpiechowicz/harness-useful-desktop/commit/56806ce71c9de612b6ee58c6c891547959bffefc))

# [0.15.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.14.0...v0.15.0) (2026-10-15)


### Features

* **onboarding:** recommended setup step installs the dashboard, the skills and the dashboard MCP entry for every detected harness ([a81bf72](https://github.com/MSpiechowicz/harness-useful-desktop/commit/a81bf72e8355704b583bb1a4cdbe10dffb2a9dd0))
* **settings:** share setup exports and imports your extensions and MCP servers as a code or a file ([8b0495f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/8b0495f28bd80b3120d615e56a7e6819a84a47fb))

# [0.14.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.13.0...v0.14.0) (2026-10-15)


### Bug Fixes

* **chat:** centre the status pill icon in its 16px cell ([b49b017](https://github.com/MSpiechowicz/harness-useful-desktop/commit/b49b0178570719e63c4f11f55406e249f5a85547))
* **extensions:** auto-update refreshes each marketplace by name so one broken catalog no longer blocks the rest ([585013d](https://github.com/MSpiechowicz/harness-useful-desktop/commit/585013d5bbc30fc6ad047717d5ea1a0a4a0aacbc))


### Features

* **settings:** move the search field into the header bar ([7d8a6c3](https://github.com/MSpiechowicz/harness-useful-desktop/commit/7d8a6c3b31cf41185a09556391da6bfb3e24b4f8))

# [0.13.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.12.0...v0.13.0) (2026-10-15)


### Features

* **chat:** Cmd/Ctrl+Down also jumps to the latest message, for keyboards without End ([2985b78](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2985b78257e078c2a042466f7351ad6b110cd0aa))
* **extensions:** update plugins, marketplaces and packages automatically, with a setting and toasts ([#18](https://github.com/MSpiechowicz/harness-useful-desktop/issues/18)) ([04edd79](https://github.com/MSpiechowicz/harness-useful-desktop/commit/04edd79d193b6b322e473c5231eb3f2c26356b33)), closes [#12](https://github.com/MSpiechowicz/harness-useful-desktop/issues/12)

# [0.12.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.11.0...v0.12.0) (2026-10-15)


### Features

* **settings): search sections; fix(chat:** compact status pill, undimmed locked project picker ([cd756d8](https://github.com/MSpiechowicz/harness-useful-desktop/commit/cd756d873d85e217c3577f5c7555bc174fff01b4))

# [0.11.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.10.0...v0.11.0) (2026-10-15)


### Features

* **shell:** Cmd+Shift+B opens the list drawer of Sessions, Changes, Config and Extensions; next and previous chat use Cmd+Option+Arrow on macOS ([25b5f2e](https://github.com/MSpiechowicz/harness-useful-desktop/commit/25b5f2e915c1e1a5618677024631ba0c782752c8))

# [0.10.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.9.0...v0.10.0) (2026-10-15)


### Bug Fixes

* **ui:** update policy row keeps its height across options, updates card bottom padding matches the top ([af1e679](https://github.com/MSpiechowicz/harness-useful-desktop/commit/af1e679801c59a3a4087dd5beff2853c092c02d3))


### Features

* **chat:** Cost toggle in the chat header and Settings hides the model, tokens and cost under replies ([f85f1cf](https://github.com/MSpiechowicz/harness-useful-desktop/commit/f85f1cf5fbd5fc3a13d5eefebb86f53e84b06e2c))
* **ui:** thinking and tool calls share one row design with an icon tile and a fixed-width status pill, spinner matches the icon geometry ([53512cd](https://github.com/MSpiechowicz/harness-useful-desktop/commit/53512cdbdee04b4087d56ea54206188987aa1dac))

# [0.9.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.8.1...v0.9.0) (2026-10-15)


### Bug Fixes

* **ui:** pickers focus their search field and draw the row focus ring inside, project picker drops the branch, update bar spans the card ([719efea](https://github.com/MSpiechowicz/harness-useful-desktop/commit/719efea3e598716672a42fac373ac5bc0266048f))


### Features

* **chat:** queued messages list above the composer with a remove button per message and Alt+Up to take the latest back (OMP remove_queued_message) ([3122a72](https://github.com/MSpiechowicz/harness-useful-desktop/commit/3122a7260159e84e9357ea6191a63323254e93ea))

## [0.8.1](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.8.0...v0.8.1) (2026-10-15)


### Bug Fixes

* **ui:** VirtualList leaves follow mode only on a scroll up the person asked for, with follow-mode tests ([93a114c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/93a114cda98e57051e6424108a561bfeed585c66))

# [0.8.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.7.0...v0.8.0) (2026-10-15)


### Features

* **ui:** spinner and close button share one slot in the chat tree, scroll-to-latest shows its shortcut, rail badge grows toward the left ([16aa4e0](https://github.com/MSpiechowicz/harness-useful-desktop/commit/16aa4e0ff6dd0f0d74c481dd2e1ef2d0c6aa786e))

# [0.7.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.6.0...v0.7.0) (2026-10-15)


### Bug Fixes

* **ci:** format the discard code, read test files with LF endings for Windows, share the git test setup ([984ca17](https://github.com/MSpiechowicz/harness-useful-desktop/commit/984ca170df9e1d7cd2204d5b179341727b091172))
* **ui:** ListDetail renders its split once, clearing the duplication guard ([b07f7ad](https://github.com/MSpiechowicz/harness-useful-desktop/commit/b07f7ade452dc34e0d2b55a9b5b3a51c05ec830c))


### Features

* **changes:** discard one file or every change from the More actions menu, show only the file name in the diff header ([38b4251](https://github.com/MSpiechowicz/harness-useful-desktop/commit/38b4251f44bb82079e4f661b7d6f08f20fa1f640))
* **ui:** list pane becomes a drawer on narrow windows in Sessions, Changes, Config and Extensions ([006fa9c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/006fa9c0115d0ad2a1fe2c7ae408a7551eb78806))

# [0.6.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.5.0...v0.6.0) (2026-10-14)


### Features

* **chat:** switch harness from the chat, replace the rail switcher, handle a harness with no models ([c4f743b](https://github.com/MSpiechowicz/harness-useful-desktop/commit/c4f743bdfdbc62d6b0c6036bbc7a19693bf6acb5))
* **extensions:** separate Refresh catalog from Update, update from the catalog row, reload chats after a plugin change ([cabd0a4](https://github.com/MSpiechowicz/harness-useful-desktop/commit/cabd0a4f837286fad76f9084fabe70cf3da1def7))

# [0.5.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.4.0...v0.5.0) (2026-10-14)


### Bug Fixes

* **chat:** an open tool card's header has square bottom corners so its hover wash fills the divider ([eaddb3c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/eaddb3c91c3c1e11805c759664a542272f17221b))
* **chat:** Attach is styled like the pickers beside it ([3b2626e](https://github.com/MSpiechowicz/harness-useful-desktop/commit/3b2626e7255bb48e68a995e36dca8367d6b5afa3))
* **chat:** even out the optical padding of the Attach button ([1aa7129](https://github.com/MSpiechowicz/harness-useful-desktop/commit/1aa7129aa746ae4151f7ef47698280349d4814b3))
* **chat:** footer text ends under the send button's right edge ([1354db0](https://github.com/MSpiechowicz/harness-useful-desktop/commit/1354db08c738bfe0f860af9c8ebb6cd714ba2111))
* **chat:** footer text shares one baseline; the branch is plain text, only the changes count opens Changes ([ed69a96](https://github.com/MSpiechowicz/harness-useful-desktop/commit/ed69a96d6f0ea10077cf2af7e2f3c487467dc615))
* **chat:** lift the chat so the composer's bottom edge meets the language picker's top edge ([3291c3c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/3291c3c46ba039ee3186b142aebe54d8128349ef))
* **chat:** lift the composer so its top lines up with the rail footer's top border ([2542b85](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2542b859f586ccc8b2a783b445d6a486d8654657))
* **chat:** the attached file chip in a message uses the standard 12px corner ([cd36185](https://github.com/MSpiechowicz/harness-useful-desktop/commit/cd361857df8b810d6004dd982b7e974738c31331))
* **chat:** the changes count in the card footer uses the Changes view's icon ([83cdbbb](https://github.com/MSpiechowicz/harness-useful-desktop/commit/83cdbbbb60e3cfd33f8dc2408a711714ead22eeb))
* **chat:** the send button sits as far from the card edge as the paperclip ([a24d5ae](https://github.com/MSpiechowicz/harness-useful-desktop/commit/a24d5ae79e5491faf7be3ca7e35d58994f1d8f37))
* **ci:** update rustls past four advisories and remove the duplicated markup the 0% clone guard found ([343b8e9](https://github.com/MSpiechowicz/harness-useful-desktop/commit/343b8e9023744d0ec0cceee767d6cfe703b4b67e))
* **sessions:** the fork button is as tall as the message beside it ([24595b0](https://github.com/MSpiechowicz/harness-useful-desktop/commit/24595b03e1c8b808693e5a521b48879ffdec7451))
* **sessions:** the transcript uses the chat column, 880px like the composer card ([0f1dca8](https://github.com/MSpiechowicz/harness-useful-desktop/commit/0f1dca80fffcbe2532ceab4767942aee8e446c5a))
* **updates:** Install is the wide button, Later a quiet ghost button ([c85f255](https://github.com/MSpiechowicz/harness-useful-desktop/commit/c85f2550714357a519165b901efa773018dd51ec))
* **updates:** small rail buttons use the 8px corner ([b0bcb6f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/b0bcb6f91fbb537825447626ae4bbf95a1dcf275))
* **updates:** the update card uses the rail footer's 8px corner ([3aacd5f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/3aacd5f460988aa05129df37b46df76f05ceceac))


### Features

* **changes:** rebuilt Changes view with sections, search, expandable diff gaps and a review dialog ([bfa94fc](https://github.com/MSpiechowicz/harness-useful-desktop/commit/bfa94fc8c42122efc53dfb3a29d32237068f9f55))
* **chat:** a new chat's placeholder name is a random free creature, not always the first ([5d1e6bb](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5d1e6bb350bf2cd21390fc32168e5c0214067d1c))
* **chat:** composer card with project, branch, changes, context and cost; Steer/Queue; Esc Esc stops ([9674e2f](https://github.com/MSpiechowicz/harness-useful-desktop/commit/9674e2ff6289f5a63b7a4d63fe24269e9adee12f))
* **chat:** placeholder chat names combine an adjective and a creature, 1800 in English, agreeing in gender in de/es/fr/pl ([b6d8aa5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/b6d8aa5f0af1684dfe19c9eac39bcfd45f23fdec))
* **chat:** project picker with search and an edit mode to remove projects from the list ([fc9d6b3](https://github.com/MSpiechowicz/harness-useful-desktop/commit/fc9d6b38d1c494e7e3cf8e8ae5dae1a6588889ef))
* **chat:** send and stop are rounded squares with bigger icons; the Steer/Queue switch follows ([e3abdc9](https://github.com/MSpiechowicz/harness-useful-desktop/commit/e3abdc926dc4d642411172ef009a2b748ad1ae2e))
* **config:** Configuration view rebuilt on the Sessions and Changes shell with Structured and Source modes ([fe52211](https://github.com/MSpiechowicz/harness-useful-desktop/commit/fe52211a9b8c706fc0f8f5cad991d2cf8f6e53a8))
* **extensions:** one grouped inventory replaces the tabs, with GitHub-aware update checks ([25ae6c7](https://github.com/MSpiechowicz/harness-useful-desktop/commit/25ae6c74e4619b0acaae7fb424f910b452c92613))
* **git:** read a changed file's lines so the diff can open the gaps between hunks ([228e012](https://github.com/MSpiechowicz/harness-useful-desktop/commit/228e0123e7c8d7e917ffc7283b47ff08e6b36caa))
* **sessions:** a visible Fork from here button under every user message ([ca1a0a0](https://github.com/MSpiechowicz/harness-useful-desktop/commit/ca1a0a074cd6c9d9a961390c4bcb2275420cd507))
* **sessions:** date sections, one-line rows and the actions in the top bar ([b83775c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/b83775c08760332ad18277bc4ecd9b49b74b4d4b))
* **shell:** remove the status bar; the rail footer shows Working and the update indicator ([524ce56](https://github.com/MSpiechowicz/harness-useful-desktop/commit/524ce56ed03230a737a6230af7ba8a5354f55c83))
* **ui:** empty-state pages for Changes and Sessions, plainer Profiles card ([f07c7af](https://github.com/MSpiechowicz/harness-useful-desktop/commit/f07c7af601f7138601b012cda3a9ad159f9343d6))


### Reverts

* the fork button goes back to the standard control size ([5f17cf7](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5f17cf7584fa75b6fd278523e45e293ac471e38a))

# [0.4.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.3.0...v0.4.0) (2026-10-14)


### Bug Fixes

* **sessions:** bounded agent startup, project-chosen session folders shown on delete ([8fb9c81](https://github.com/MSpiechowicz/harness-useful-desktop/commit/8fb9c8116405c5a548f12605aebac5edd7e872b0))
* **ui:** use explicit 52px top bar height to match dashboard ([d871e75](https://github.com/MSpiechowicz/harness-useful-desktop/commit/d871e75e54a7eb1cbbd9dfb1cedf4899fa2ce54c))
* **ui:** use explicit pixel sizes for Switch track and knob travel ([9277c5d](https://github.com/MSpiechowicz/harness-useful-desktop/commit/9277c5d9b332640f7f3a58a6694f9df06db00e19))


### Features

* **chat:** attach images and files to a message ([#10](https://github.com/MSpiechowicz/harness-useful-desktop/issues/10)) ([977c7e7](https://github.com/MSpiechowicz/harness-useful-desktop/commit/977c7e7b334c47b0cf49d8baf718b608284ab2b6))
* **ecosystem:** cache card with hit rate, token sources and per-model rates ([344ccf2](https://github.com/MSpiechowicz/harness-useful-desktop/commit/344ccf2fdc830ab3e01029d591df156e769ac67a)), closes [#13](https://github.com/MSpiechowicz/harness-useful-desktop/issues/13)
* **sessions:** redesigned view, one agent per session, delete and clean-up ([2df8665](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2df86657ba069912b8436c2cd3c1742c1fb12a67))
* **shell:** Ctrl+End jumps to the latest message; shortcuts in the rail menu, even key caps, aligned icons ([f239586](https://github.com/MSpiechowicz/harness-useful-desktop/commit/f2395862f6eb33b6535530d7b238f17978274b47))
* **ui:** chat controls move to the top bar, cost joins the context meter ([5e984f5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5e984f57fc1ce1256d26973602b35d070cac7711))
* **ui:** chats live in a tree under Chat, named by a generator or AI ([8773c13](https://github.com/MSpiechowicz/harness-useful-desktop/commit/8773c134a41db609d9bcac5d4dc2035b7a0c44e9))
* **ui:** mirror dashboard's compact update card in Settings ([be93195](https://github.com/MSpiechowicz/harness-useful-desktop/commit/be93195fe29ce9950a21cdbb1965c2a3495660f0))
* **ui:** setting descriptions cap at 700px and can list key points; reword AI chat names ([9c51d9b](https://github.com/MSpiechowicz/harness-useful-desktop/commit/9c51d9b9cb8543e7be39661c2e90c8300a36705e))
* **ui:** the open chat sets the project; AI chat names survive a restart ([c9a2468](https://github.com/MSpiechowicz/harness-useful-desktop/commit/c9a24683564b32dfae12cf7ae44edc8b81f5d758))

# [0.3.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.2.0...v0.3.0) (2026-10-12)


### Features

* the app speaks English, German, Spanish, French and Polish, and the sidebar matches the dashboard ([e324085](https://github.com/MSpiechowicz/harness-useful-desktop/commit/e324085090315b5f5e1854bfdd86299c4d574b74))

# [0.2.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.1.7...v0.2.0) (2026-10-12)


### Bug Fixes

* Changes and Sessions follow the agent while it works ([93a1bfd](https://github.com/MSpiechowicz/harness-useful-desktop/commit/93a1bfd371a8b70a0087690379f437364b73adc9))


### Features

* an "Open any web link" setting lets links from chats open ([30cdfb7](https://github.com/MSpiechowicz/harness-useful-desktop/commit/30cdfb7a5d7167581b59306f70e4bf66de630edf))

## [0.1.7](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.1.6...v0.1.7) (2026-10-12)


### Bug Fixes

* the update band shows download progress on one line ([6d46479](https://github.com/MSpiechowicz/harness-useful-desktop/commit/6d4647947d30c82e3251ca9c5373f4607d406552))

## [0.1.6](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.1.5...v0.1.6) (2026-10-12)


### Bug Fixes

* agents start when the app is opened from the Finder and the harness is a bun or node script ([ebc098d](https://github.com/MSpiechowicz/harness-useful-desktop/commit/ebc098d60874aeddd69f4a02d29db852e34811e7))

## [0.1.5](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.1.4...v0.1.5) (2026-10-11)


### Bug Fixes

* the composer and user messages get the same 2px outline as the chat cards ([bd8dc00](https://github.com/MSpiechowicz/harness-useful-desktop/commit/bd8dc00654ff1c97a09ca88da14880ae25bce938))
* tool and thinking cards get a 2px outline on 1x screens ([9901516](https://github.com/MSpiechowicz/harness-useful-desktop/commit/990151662555b2ce568fe5fe7df66c0a23ccf076))

## [0.1.4](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.1.3...v0.1.4) (2026-10-11)


### Bug Fixes

* failed tool cards get a red border, not a purple one ([72be896](https://github.com/MSpiechowicz/harness-useful-desktop/commit/72be896505bbdadc16b39d1bd3577f1cacdb59a2)), closes [#623b5f](https://github.com/MSpiechowicz/harness-useful-desktop/issues/623b5f) [#d8a2d0](https://github.com/MSpiechowicz/harness-useful-desktop/issues/d8a2d0)

## [0.1.3](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.1.2...v0.1.3) (2026-10-11)


### Bug Fixes

* reopening the last run's chats no longer fails on a chat that was never used ([48e1858](https://github.com/MSpiechowicz/harness-useful-desktop/commit/48e1858bafb63e324e44db912b164d48955c05f3))

## [0.1.2](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.1.1...v0.1.2) (2026-10-11)


### Bug Fixes

* extension notices, status, widgets and titles show without terminal color codes ([2b919f4](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2b919f40d377dc0fced43ff5757e54100f432a83))
* **linux:** agents started from the AppImage get the host environment ([015c67d](https://github.com/MSpiechowicz/harness-useful-desktop/commit/015c67d6f489703d8a84ed7dc6fd5a2e9b924865))
* moving a chat to another project is allowed by the ACL ([92f4a12](https://github.com/MSpiechowicz/harness-useful-desktop/commit/92f4a127c029447d1f7d04b31e896c6ca76ba130))

## [0.1.1](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.1.0...v0.1.1) (2026-10-11)


### Bug Fixes

* **linux:** the AppImage no longer crashes at launch on current distributions ([52825d5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/52825d5b3c63ba3eb175a670ce937a472c967e31))

# [0.1.0](https://github.com/MSpiechowicz/harness-useful-desktop/compare/v0.0.0...v0.1.0) (2026-10-11)


### Bug Fixes

* a crisp composer border, a blue-tinted message bubble border, and sharp diff text ([75f2f41](https://github.com/MSpiechowicz/harness-useful-desktop/commit/75f2f4105024f15ef3cc68f70d11641b57a2d382))
* a crisp glowing status dot, one flat workspace menu, fixed search in the header ([aaa275e](https://github.com/MSpiechowicz/harness-useful-desktop/commit/aaa275e459d6f449234ee0d4615c1cf5a75ae39b))
* anti-aliased, left-aligned "How it works" timeline ([3560c51](https://github.com/MSpiechowicz/harness-useful-desktop/commit/3560c514541401724dff7fab36ca4ad4e3ec116f))
* aurora keeps drifting behind the shell ([045a900](https://github.com/MSpiechowicz/harness-useful-desktop/commit/045a900184ea17f7a8fda75b9e5a8869c3603d46))
* centered rail icons, no gradient bar beside the active tile ([c16c7c3](https://github.com/MSpiechowicz/harness-useful-desktop/commit/c16c7c3e575f49770a05f6fe73bae408b4a0c1b9))
* **ci:** case-distinct state module names for macOS and Windows, and Windows-safe test fixtures ([bfb85e0](https://github.com/MSpiechowicz/harness-useful-desktop/commit/bfb85e0a44c0989e94e1ea8158413d9a1fdf5308))
* **ci:** desktop test temp dirs canonicalize like stored projects on Windows ([710d037](https://github.com/MSpiechowicz/harness-useful-desktop/commit/710d0374263b689413f2c1b68f152b502282bbb0))
* **ci:** drop the job-level hashFiles guard that made every CI run fail to parse ([df77646](https://github.com/MSpiechowicz/harness-useful-desktop/commit/df77646736e5d57b1ebdc4f7cc8d2fbe3fe02cc3))
* **ci:** install ALSA headers for the launch sound, and make two tests hold on Windows ([2fcea13](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2fcea130412e6d63f624fa467e7125c900b09a9d))
* Ecosystem page crashed on per-model plan limits ([22b1bc6](https://github.com/MSpiechowicz/harness-useful-desktop/commit/22b1bc64345319911de059c4ba03448137b337d0))
* even spacing in the project card tiles and the timeline ([fd8d64d](https://github.com/MSpiechowicz/harness-useful-desktop/commit/fd8d64d7a9b3f67010ef45a807a7781931056074))
* higher-quality glow for selected cards and the rail highlight ([016d65a](https://github.com/MSpiechowicz/harness-useful-desktop/commit/016d65a27fa5d862bcb5ac90eb3e0e8cf134e0f4)), closes [hi#alpha](https://github.com/hi/issues/alpha)
* one brand, one title, one place for everything in the shell ([9deb2a1](https://github.com/MSpiechowicz/harness-useful-desktop/commit/9deb2a1beb90982172c56d4602587cbaf629499c))
* one top bar, matching bar heights, a live status dot ([258a7a5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/258a7a576066cd8037631bf1934d227832c49df4))
* optically even button padding, a blue info tip ([e52bd91](https://github.com/MSpiechowicz/harness-useful-desktop/commit/e52bd913f2d6fea397144f2b93bd85d9a97b12bf))
* PI install hint uses the official pi.dev install script ([108504b](https://github.com/MSpiechowicz/harness-useful-desktop/commit/108504ba7f860c52b40a3c6882403550bf1713e5))
* plan limits show the two plans in use, named once, with a header that wraps ([b53b563](https://github.com/MSpiechowicz/harness-useful-desktop/commit/b53b563d05a34de482e0180b7f08bf0a7aac82ca))
* project step is one card, single border, no competing project list ([0f16988](https://github.com/MSpiechowicz/harness-useful-desktop/commit/0f16988b71431efb89e152d2cebad911c285e4d5))
* project step matches concept A and its lower half gets real panels ([032c5d5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/032c5d5c95e3181be2fd1e956607d2ab5a177198))
* refresh is a labelled button in every view header, gone from the branches panel ([eb463bb](https://github.com/MSpiechowicz/harness-useful-desktop/commit/eb463bb1f3f8c2d0e9070babaa612bf30c953ea6))
* repeat end aligns with the steps; folder picker opens among projects ([d3a1810](https://github.com/MSpiechowicz/harness-useful-desktop/commit/d3a18104c7bcde317a36f3a4fb060056700f22f7))
* review feedback on hover jitter, welcome spacing, crack timing and Linux icon ([cff473b](https://github.com/MSpiechowicz/harness-useful-desktop/commit/cff473b63792300a8bfc56f718e60c2040f40f90))
* selected card edge is the accent blue again, not white ([5214926](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5214926c0894ca239560d7882d6ae025ce9d06f4))
* smoke no longer clipped, folder picker opens where projects live, setup timeline ([2093e66](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2093e66f1171ed45a06cb0bfac29546281156e06))
* smooth rounded accent edges on selected cards ([77af0ec](https://github.com/MSpiechowicz/harness-useful-desktop/commit/77af0ece536fb374bc9abe48dc2ddad8258a362d))
* smooth wizard transitions, rebuilt project picker, roomier key caps ([5185acf](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5185acf8b59f2c1de0024d92340c60808887cc41))
* stable onboarding frame, wizard buttons, radio and wordmark spacing ([9789371](https://github.com/MSpiechowicz/harness-useful-desktop/commit/97893718de69444541ca2be12905a9cb9fb78ab3))
* the actions tip carries an info icon ([9b0f0a5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/9b0f0a5b5e62fb0e86fa33291bf06ae63b664710))
* the config section is one column, nothing floats beside the tree ([265beb0](https://github.com/MSpiechowicz/harness-useful-desktop/commit/265beb0b650a5a364d8a8d1a8e58e945bb70c817))
* the dev desktop entry gives each logo its own icon name ([7d0858c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/7d0858ccfd0d458c912a026b6b52ee3e736ae9d5))
* the extension detail is one column, facts do not repeat the badges ([c3a6213](https://github.com/MSpiechowicz/harness-useful-desktop/commit/c3a621381c04692249a80587f1c2e7007209051a))
* wizard card shadows and glows are no longer cut off by the scroll area ([2d49832](https://github.com/MSpiechowicz/harness-useful-desktop/commit/2d49832a943c37eaf823a86523d0b5569d87807f))


### Features

* "How it works" timeline closes the loop across the full width ([4508354](https://github.com/MSpiechowicz/harness-useful-desktop/commit/450835431a7ca77633e8061ba1024a2c6b4be73c))
* "Something went wrong" fallback for every page and layer ([20adebb](https://github.com/MSpiechowicz/harness-useful-desktop/commit/20adebb3359991c1d396d2c01efabbea41650ba8))
* chats move between projects in place, the app reopens where it left off, and reviews from any diff line ([41528b8](https://github.com/MSpiechowicz/harness-useful-desktop/commit/41528b89052d0bd5e4b2ade32780a113ef82c6c8))
* choreographed wizard transitions, distinct project step, uniform key caps ([5a5836a](https://github.com/MSpiechowicz/harness-useful-desktop/commit/5a5836aacda8a4b094b9126edceab4a85f3c52a9))
* companion update checks on GitHub, and an Ecosystem view polish pass ([135fb28](https://github.com/MSpiechowicz/harness-useful-desktop/commit/135fb285b4204218f3b9dcc65df2e236f61ec6bb))
* four-step "How it works" timeline across the full width ([0cccf6e](https://github.com/MSpiechowicz/harness-useful-desktop/commit/0cccf6e48ef469f8a85136c9c517ad98cb196ade))
* Graphite theme, the shared family logo with sound, a tray icon, and a UI polish pass ([67b2556](https://github.com/MSpiechowicz/harness-useful-desktop/commit/67b255618c94d9b78ee56552c098beb6c9fd5046))
* impact dust driven out from under the landing slab ([d037558](https://github.com/MSpiechowicz/harness-useful-desktop/commit/d03755801d917f1492ad81219d8b97d9aa643cfc))
* live subagent cards, a chat-first shell and a polish pass across chat and sessions ([6e0597c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/6e0597c5c1bb4e8a9ede5680ffc0d57cfdab1957))
* logo lab for looping, slowing and scrubbing the launch animation ([a10de0e](https://github.com/MSpiechowicz/harness-useful-desktop/commit/a10de0e1a20d76774011325ec9a7b929a2a2cbd2))
* M0 foundation for Harness Desktop ([48bc02c](https://github.com/MSpiechowicz/harness-useful-desktop/commit/48bc02ccb2cc4933d079bdec9174ade782b38e59))
* M1 harness adapters, config viewer and onboarding ([38d7c13](https://github.com/MSpiechowicz/harness-useful-desktop/commit/38d7c135bfa357e262169b6b5dbc5176c2bb5309))
* M2 native agent chat over OMP and PI RPC ([14b0032](https://github.com/MSpiechowicz/harness-useful-desktop/commit/14b00326cee737e5a098ef2f6673ea8f5fff8d42))
* M3 sessions, branches and changes panel with review comments ([28ce659](https://github.com/MSpiechowicz/harness-useful-desktop/commit/28ce659c628d3e6f70af44af4a8ba6fad2147046))
* M4 ecosystem and M5 updates, release pipeline and docs ([fdf5cf2](https://github.com/MSpiechowicz/harness-useful-desktop/commit/fdf5cf268216100d47a3bb69dbda9ad03108f293))
* onboarding steps 2-5 share one layout and carry more guidance ([327623b](https://github.com/MSpiechowicz/harness-useful-desktop/commit/327623bc79c101119d2a106b436ce97831124c40))
* one-line installers for macOS, Linux and Windows, with uninstall steps in the README ([0310632](https://github.com/MSpiechowicz/harness-useful-desktop/commit/0310632ee55a95893a000d6e17061db97c2ecd1d))
* project step as a featured project card with context underneath ([1ab49f5](https://github.com/MSpiechowicz/harness-useful-desktop/commit/1ab49f535cdf1de0b1fe259d8afc0128e4b4bfcf))
* splintered break for the H_ mark, crack dust and glyph shards ([6294bff](https://github.com/MSpiechowicz/harness-useful-desktop/commit/6294bff22a3801b9476ed2c5bf803095eb5eecef))
* Upgrade only when the marketplace has a newer version; focus rings stay inside ([fb1703a](https://github.com/MSpiechowicz/harness-useful-desktop/commit/fb1703a10140b23f75b4ef060c0f0d1e5529fed4))
* UX-2 polish for intro, chat chrome and onboarding ([1d80f45](https://github.com/MSpiechowicz/harness-useful-desktop/commit/1d80f4529db0dcb9353e3daa6651180107dad944))
