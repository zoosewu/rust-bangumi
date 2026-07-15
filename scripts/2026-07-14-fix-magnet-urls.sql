-- 2026-07-14 資料修復：magnet URL → 原始 mikanani .torrent URL
--
-- 背景：fetcher 曾把 mikanani .torrent URL 轉成僅含 hash + 3 個已死 tracker 的
-- magnet link（fetchers/mikanani rss_parser 舊行為），導致 qBittorrent 無 peer
-- 來源、下載卡 0%。fetcher 已改為保留原始 .torrent URL；本腳本把既有資料改寫成
-- 相同格式，讓 raw_items 去重（download_url UNIQUE）與後續 RSS 抓取保持一致，
-- 並將卡住的下載重置為 failed，交由 downloader 註冊時的自動重試以新 URL 重新派送。
--
-- 映射來源：2026-07-14 自 7 個訂閱的 RSS feed 實際收割（與 DB 中 190 個 magnet
-- 的 btih hash 100% 對應）。冪等：重跑無副作用。
--
-- 使用方式（生產）：
--   docker exec -i bangumi-postgres psql -U bangumi -d bangumi -v ON_ERROR_STOP=1 < 本檔
BEGIN;

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TEMP TABLE magnet_url_fix (hash TEXT PRIMARY KEY, new_url TEXT NOT NULL) ON COMMIT DROP;
INSERT INTO magnet_url_fix (hash, new_url) VALUES
  ('762b4faa68d143f78adea5857b0cabb9df10a25a', 'https://mikanani.me/Download/20180228/762b4faa68d143f78adea5857b0cabb9df10a25a.torrent'),
  ('a672a20ee86b24fef9c6b840b31141803c47f120', 'https://mikanani.me/Download/20180228/a672a20ee86b24fef9c6b840b31141803c47f120.torrent'),
  ('be378ab06204e819ea35a0a8a4b429f1bc41e9f6', 'https://mikanani.me/Download/20180228/be378ab06204e819ea35a0a8a4b429f1bc41e9f6.torrent'),
  ('cc9e736792bb40ab7822a5b9054c58efaeb2a154', 'https://mikanani.me/Download/20180228/cc9e736792bb40ab7822a5b9054c58efaeb2a154.torrent'),
  ('5e7fd2b62fb74ef8d0da49a7bf1bd7916b39624a', 'https://mikanani.me/Download/20180512/5e7fd2b62fb74ef8d0da49a7bf1bd7916b39624a.torrent'),
  ('28ec5f37de7ee39bb6aec098dddbf7a8c75bc357', 'https://mikanani.me/Download/20180618/28ec5f37de7ee39bb6aec098dddbf7a8c75bc357.torrent'),
  ('2c41f2bf8be6ff45b206d71426246bf16097e89d', 'https://mikanani.me/Download/20180618/2c41f2bf8be6ff45b206d71426246bf16097e89d.torrent'),
  ('82db775424b409f3f5593a7f129163a126314a5b', 'https://mikanani.me/Download/20180618/82db775424b409f3f5593a7f129163a126314a5b.torrent'),
  ('8a335cdcc9278d04337529a0eef4eaf30aef5cba', 'https://mikanani.me/Download/20180618/8a335cdcc9278d04337529a0eef4eaf30aef5cba.torrent'),
  ('87a97a59a94d7cafb2eead46422accc62425bfc1', 'https://mikanani.me/Download/20230402/87a97a59a94d7cafb2eead46422accc62425bfc1.torrent'),
  ('a35027ff98d07c0228f1b21a6c4dce9eb1021d03', 'https://mikanani.me/Download/20230402/a35027ff98d07c0228f1b21a6c4dce9eb1021d03.torrent'),
  ('c5b3bdb1da251e4f71aadf8b6061051faa1a830b', 'https://mikanani.me/Download/20230402/c5b3bdb1da251e4f71aadf8b6061051faa1a830b.torrent'),
  ('f45544f070c05915ec44a05d5871598f8c9655b7', 'https://mikanani.me/Download/20230402/f45544f070c05915ec44a05d5871598f8c9655b7.torrent'),
  ('1d523b0bc25e620e259ec1493430d35330b34374', 'https://mikanani.me/Download/20230409/1d523b0bc25e620e259ec1493430d35330b34374.torrent'),
  ('65d301556446caaaa7b582cf8d8890542e496970', 'https://mikanani.me/Download/20230409/65d301556446caaaa7b582cf8d8890542e496970.torrent'),
  ('796ff938f8fe6810b03365a7937e214d363b622b', 'https://mikanani.me/Download/20230409/796ff938f8fe6810b03365a7937e214d363b622b.torrent'),
  ('ed36f6f47adaa5b48b0865fefe09a17032fe487c', 'https://mikanani.me/Download/20230409/ed36f6f47adaa5b48b0865fefe09a17032fe487c.torrent'),
  ('1f74c31da880fe1f5d8d1babd0be58cde95b33a5', 'https://mikanani.me/Download/20230416/1f74c31da880fe1f5d8d1babd0be58cde95b33a5.torrent'),
  ('3d5f4e902258f0c63e0c0657747a67cb87b9bd96', 'https://mikanani.me/Download/20230416/3d5f4e902258f0c63e0c0657747a67cb87b9bd96.torrent'),
  ('56ac78fef4892b7ff73b80f553f35321e5fcecb3', 'https://mikanani.me/Download/20230416/56ac78fef4892b7ff73b80f553f35321e5fcecb3.torrent'),
  ('9c555cbab94526bd6de34431180aedb30e08f434', 'https://mikanani.me/Download/20230416/9c555cbab94526bd6de34431180aedb30e08f434.torrent'),
  ('5d5fa88c42528a61e2ca00f0b307e129269964d2', 'https://mikanani.me/Download/20230423/5d5fa88c42528a61e2ca00f0b307e129269964d2.torrent'),
  ('72346708d42cde828699ac50bab9c425115b55ce', 'https://mikanani.me/Download/20230423/72346708d42cde828699ac50bab9c425115b55ce.torrent'),
  ('7b9df198daf276a800b5670985c9dea1581f66db', 'https://mikanani.me/Download/20230423/7b9df198daf276a800b5670985c9dea1581f66db.torrent'),
  ('8c695a50a61b656dd25bab41474172e13ddd2cff', 'https://mikanani.me/Download/20230423/8c695a50a61b656dd25bab41474172e13ddd2cff.torrent'),
  ('4ddf655b878c380278d7314437d63efaadbee4dd', 'https://mikanani.me/Download/20230430/4ddf655b878c380278d7314437d63efaadbee4dd.torrent'),
  ('561b5c0497bcf87c04929435210d28fde087ff7c', 'https://mikanani.me/Download/20230430/561b5c0497bcf87c04929435210d28fde087ff7c.torrent'),
  ('b5ec737cf2c40ebe8f7dfc457177ebc0035d3540', 'https://mikanani.me/Download/20230430/b5ec737cf2c40ebe8f7dfc457177ebc0035d3540.torrent'),
  ('ba4e9d21362c758f403282eb7718ec15040a8e71', 'https://mikanani.me/Download/20230430/ba4e9d21362c758f403282eb7718ec15040a8e71.torrent'),
  ('5fea74f3df139c52d8ec5a0017b8cae4417150c9', 'https://mikanani.me/Download/20230507/5fea74f3df139c52d8ec5a0017b8cae4417150c9.torrent'),
  ('a118e25654fa1b9191fc6882df7dde4637c08bed', 'https://mikanani.me/Download/20230507/a118e25654fa1b9191fc6882df7dde4637c08bed.torrent'),
  ('b4cb30b18bd1ea3656c0033579c351fe4425c0d3', 'https://mikanani.me/Download/20230507/b4cb30b18bd1ea3656c0033579c351fe4425c0d3.torrent'),
  ('b94c607af7bbf8a0fad369296933294821d95524', 'https://mikanani.me/Download/20230507/b94c607af7bbf8a0fad369296933294821d95524.torrent'),
  ('285d3f6c2ac99feb0a0920ae5fb2a317578f9ae2', 'https://mikanani.me/Download/20230514/285d3f6c2ac99feb0a0920ae5fb2a317578f9ae2.torrent'),
  ('530239bcb45c8859012f36add8bef6fda2fe792c', 'https://mikanani.me/Download/20230514/530239bcb45c8859012f36add8bef6fda2fe792c.torrent'),
  ('5c8d08784ffeaf67c1633ce7585842cd9f5f9df6', 'https://mikanani.me/Download/20230514/5c8d08784ffeaf67c1633ce7585842cd9f5f9df6.torrent'),
  ('ecad41990991fa684b427f95582e2e8a2a986680', 'https://mikanani.me/Download/20230514/ecad41990991fa684b427f95582e2e8a2a986680.torrent'),
  ('457682605e9db541cfc0ca6ab7ed749670173196', 'https://mikanani.me/Download/20230521/457682605e9db541cfc0ca6ab7ed749670173196.torrent'),
  ('8d20e479b4b3ff7d795eb8e9a3c3bc7985047e22', 'https://mikanani.me/Download/20230521/8d20e479b4b3ff7d795eb8e9a3c3bc7985047e22.torrent'),
  ('e1417947b99fe32283c28b7660e4f5bb289a70e1', 'https://mikanani.me/Download/20230521/e1417947b99fe32283c28b7660e4f5bb289a70e1.torrent'),
  ('e9e887dc4c4e17361dad4c46b7aa43218ade5764', 'https://mikanani.me/Download/20230521/e9e887dc4c4e17361dad4c46b7aa43218ade5764.torrent'),
  ('386d45e25e957bfa1d6406df96a487c6d00af1a3', 'https://mikanani.me/Download/20230604/386d45e25e957bfa1d6406df96a487c6d00af1a3.torrent'),
  ('58a8a0e12f7cfb11525382c21b361ded927155f1', 'https://mikanani.me/Download/20230604/58a8a0e12f7cfb11525382c21b361ded927155f1.torrent'),
  ('88aa7ed396466a855f8129f695ba3d8fe911cdf8', 'https://mikanani.me/Download/20230604/88aa7ed396466a855f8129f695ba3d8fe911cdf8.torrent'),
  ('a49ceaea7203baecbda20cc33c0dd6ccbe1e93ef', 'https://mikanani.me/Download/20230604/a49ceaea7203baecbda20cc33c0dd6ccbe1e93ef.torrent'),
  ('1af3cb797ec99b81fe49300bf5e16a7f13564d9b', 'https://mikanani.me/Download/20230611/1af3cb797ec99b81fe49300bf5e16a7f13564d9b.torrent'),
  ('2035f070d8cdb7c4337bea608611e05f4b142fda', 'https://mikanani.me/Download/20230611/2035f070d8cdb7c4337bea608611e05f4b142fda.torrent'),
  ('48b3e65af3c4a8a54e33f6a289262dd31f2528f7', 'https://mikanani.me/Download/20230611/48b3e65af3c4a8a54e33f6a289262dd31f2528f7.torrent'),
  ('e72fefbba161433fb837d71e9a197b3b38a5f60f', 'https://mikanani.me/Download/20230611/e72fefbba161433fb837d71e9a197b3b38a5f60f.torrent'),
  ('18afaf48fbce09c35b3dceeabd90f5d899f4a93e', 'https://mikanani.me/Download/20230618/18afaf48fbce09c35b3dceeabd90f5d899f4a93e.torrent'),
  ('562d4e090a211fb6e1892feae1741aba5138c895', 'https://mikanani.me/Download/20230618/562d4e090a211fb6e1892feae1741aba5138c895.torrent'),
  ('8e393efb8d2c491a0fa7158c3ebc4814e71ee4e3', 'https://mikanani.me/Download/20230618/8e393efb8d2c491a0fa7158c3ebc4814e71ee4e3.torrent'),
  ('e50f605f3f0c05287ae76f2e079b24cafe15c27c', 'https://mikanani.me/Download/20230618/e50f605f3f0c05287ae76f2e079b24cafe15c27c.torrent'),
  ('10cb1d350698895ee246d1ab171bc1217b1c99e4', 'https://mikanani.me/Download/20230625/10cb1d350698895ee246d1ab171bc1217b1c99e4.torrent'),
  ('42f32f4be4d41b57a4c054f63de61b8408f42a81', 'https://mikanani.me/Download/20230625/42f32f4be4d41b57a4c054f63de61b8408f42a81.torrent'),
  ('5c51616e42e19b25fdacff14e7d0c3ddaa6726f2', 'https://mikanani.me/Download/20230625/5c51616e42e19b25fdacff14e7d0c3ddaa6726f2.torrent'),
  ('ad8f0ea598c0294c8a31d932fefc31c3b992949a', 'https://mikanani.me/Download/20230625/ad8f0ea598c0294c8a31d932fefc31c3b992949a.torrent'),
  ('00d3471cb96ac69af84f0dc6e960720471b5e0b8', 'https://mikanani.me/Download/20230702/00d3471cb96ac69af84f0dc6e960720471b5e0b8.torrent'),
  ('483e980967dadbc16b1019746d601261c1a017ab', 'https://mikanani.me/Download/20230702/483e980967dadbc16b1019746d601261c1a017ab.torrent'),
  ('ab6fe5302a26ae57bd4654bc2beb6dc841d4de7f', 'https://mikanani.me/Download/20230702/ab6fe5302a26ae57bd4654bc2beb6dc841d4de7f.torrent'),
  ('c969fe43c718faec624c7077420ae846c6552b26', 'https://mikanani.me/Download/20230702/c969fe43c718faec624c7077420ae846c6552b26.torrent'),
  ('0e7b0aa01c62c839c9bce5f8aa999ab305eab600', 'https://mikanani.me/Download/20260130/0e7b0aa01c62c839c9bce5f8aa999ab305eab600.torrent'),
  ('86cdb77473cb02d25034ad7fd136c963cffceb11', 'https://mikanani.me/Download/20260130/86cdb77473cb02d25034ad7fd136c963cffceb11.torrent'),
  ('aac0c746184129caa84951f9fa3c112bf4b9f509', 'https://mikanani.me/Download/20260130/aac0c746184129caa84951f9fa3c112bf4b9f509.torrent'),
  ('00908557a84d713c8790950aed098d2b56849e41', 'https://mikanani.me/Download/20260205/00908557a84d713c8790950aed098d2b56849e41.torrent'),
  ('643d4dc9a138bf4043a409110093b880edff3211', 'https://mikanani.me/Download/20260205/643d4dc9a138bf4043a409110093b880edff3211.torrent'),
  ('a174982ab662fa10ded610dae9f6a82b255a53e6', 'https://mikanani.me/Download/20260205/a174982ab662fa10ded610dae9f6a82b255a53e6.torrent'),
  ('2a91500faf878c9ecc6f5cf528d1489e1f1c5f06', 'https://mikanani.me/Download/20260213/2a91500faf878c9ecc6f5cf528d1489e1f1c5f06.torrent'),
  ('53be33b3dbf64c49286b92f09ac5c7fb527a5c89', 'https://mikanani.me/Download/20260213/53be33b3dbf64c49286b92f09ac5c7fb527a5c89.torrent'),
  ('c67ddae0c61f1a50c291b27a300f80d5f512b164', 'https://mikanani.me/Download/20260213/c67ddae0c61f1a50c291b27a300f80d5f512b164.torrent'),
  ('20323aa96bc9761d2978c9fddfcce7ac81cf3a03', 'https://mikanani.me/Download/20260215/20323aa96bc9761d2978c9fddfcce7ac81cf3a03.torrent'),
  ('32c9014012133135e1e115fab1dac216ff6a790f', 'https://mikanani.me/Download/20260215/32c9014012133135e1e115fab1dac216ff6a790f.torrent'),
  ('7f6e820676f6031064fba0bc681966fc66056e37', 'https://mikanani.me/Download/20260215/7f6e820676f6031064fba0bc681966fc66056e37.torrent'),
  ('57fd061a2dc9d6ee8c2b4706e944f92a28149bf6', 'https://mikanani.me/Download/20260224/57fd061a2dc9d6ee8c2b4706e944f92a28149bf6.torrent'),
  ('6c43e476c654f48624520693bf9134cdfc03ac10', 'https://mikanani.me/Download/20260224/6c43e476c654f48624520693bf9134cdfc03ac10.torrent'),
  ('8283ecef43eff6c9a955a95e60ac2bdcfe428d0f', 'https://mikanani.me/Download/20260224/8283ecef43eff6c9a955a95e60ac2bdcfe428d0f.torrent'),
  ('20d07c9ce10747c4c60dc8399289c80e1184edbf', 'https://mikanani.me/Download/20260305/20d07c9ce10747c4c60dc8399289c80e1184edbf.torrent'),
  ('9197c5f8f4ca9d928d42b47482828eb373d18da2', 'https://mikanani.me/Download/20260305/9197c5f8f4ca9d928d42b47482828eb373d18da2.torrent'),
  ('ab946516c1b998d5004598bba2678ef83c2b7f53', 'https://mikanani.me/Download/20260305/ab946516c1b998d5004598bba2678ef83c2b7f53.torrent'),
  ('bcc9523b0f2e8abfe23284e3bcd4bef8f78d9479', 'https://mikanani.me/Download/20260309/bcc9523b0f2e8abfe23284e3bcd4bef8f78d9479.torrent'),
  ('eb36bacd82b73df0e12d8752161814ff63ec6cae', 'https://mikanani.me/Download/20260309/eb36bacd82b73df0e12d8752161814ff63ec6cae.torrent'),
  ('f38dab46de5b9ec7d79664573afd52103cf5de8c', 'https://mikanani.me/Download/20260309/f38dab46de5b9ec7d79664573afd52103cf5de8c.torrent'),
  ('54dae1162430092944018413a44dafc8ad50ea2d', 'https://mikanani.me/Download/20260319/54dae1162430092944018413a44dafc8ad50ea2d.torrent'),
  ('93b8dfe09f29bb3e1e144b4a710d64109dbde04d', 'https://mikanani.me/Download/20260319/93b8dfe09f29bb3e1e144b4a710d64109dbde04d.torrent'),
  ('dffe32df4deb72e0d8194c6680cf0cee2f6ea45f', 'https://mikanani.me/Download/20260319/dffe32df4deb72e0d8194c6680cf0cee2f6ea45f.torrent'),
  ('2bd0da7c60a389318d2ae95d83bf704cad405dbe', 'https://mikanani.me/Download/20260322/2bd0da7c60a389318d2ae95d83bf704cad405dbe.torrent'),
  ('55619d9c680b6553121633d18030fd0fab6eaa72', 'https://mikanani.me/Download/20260322/55619d9c680b6553121633d18030fd0fab6eaa72.torrent'),
  ('68162afc5223752515087863effe6fefdb6b0945', 'https://mikanani.me/Download/20260322/68162afc5223752515087863effe6fefdb6b0945.torrent'),
  ('5587dc20d1c363cb276524d337c2d7f3745a1af6', 'https://mikanani.me/Download/20260406/5587dc20d1c363cb276524d337c2d7f3745a1af6.torrent'),
  ('a1db5a1f07c750467f2e5e224b77818c31b99138', 'https://mikanani.me/Download/20260406/a1db5a1f07c750467f2e5e224b77818c31b99138.torrent'),
  ('a2fd330dae38894d7aa9e41f0b7d03d90334490a', 'https://mikanani.me/Download/20260406/a2fd330dae38894d7aa9e41f0b7d03d90334490a.torrent'),
  ('b8bebf34308af307d916202cf04c55fceb7a6d2e', 'https://mikanani.me/Download/20260406/b8bebf34308af307d916202cf04c55fceb7a6d2e.torrent'),
  ('b25e1974e9e003e16283e91ad168ef898c670bf0', 'https://mikanani.me/Download/20260408/b25e1974e9e003e16283e91ad168ef898c670bf0.torrent'),
  ('f1a733eed8c110b6b0ded3473c24b7be1fb0c9eb', 'https://mikanani.me/Download/20260408/f1a733eed8c110b6b0ded3473c24b7be1fb0c9eb.torrent'),
  ('3678e2e545f4a97e33008ff48ea5f48c1e34cbfb', 'https://mikanani.me/Download/20260410/3678e2e545f4a97e33008ff48ea5f48c1e34cbfb.torrent'),
  ('bc74eb12ec4776f5606037f498c75692db2df462', 'https://mikanani.me/Download/20260410/bc74eb12ec4776f5606037f498c75692db2df462.torrent'),
  ('18e3b1fc4e7a21427cf15fbdc741cc0c73c61d5f', 'https://mikanani.me/Download/20260413/18e3b1fc4e7a21427cf15fbdc741cc0c73c61d5f.torrent'),
  ('1bb0f9124f026182845ad3223def49b02fe9c297', 'https://mikanani.me/Download/20260413/1bb0f9124f026182845ad3223def49b02fe9c297.torrent'),
  ('3f132624bc3145b9725f36fe499e7f8e59a2830a', 'https://mikanani.me/Download/20260413/3f132624bc3145b9725f36fe499e7f8e59a2830a.torrent'),
  ('5c9668b76b9e3e046066d700c4d2e37a84c0ff69', 'https://mikanani.me/Download/20260413/5c9668b76b9e3e046066d700c4d2e37a84c0ff69.torrent'),
  ('24d736843a6671c34de7f4fb18769e171420c895', 'https://mikanani.me/Download/20260415/24d736843a6671c34de7f4fb18769e171420c895.torrent'),
  ('47770397cb1b6eb39492365674f995e4ad38c82a', 'https://mikanani.me/Download/20260415/47770397cb1b6eb39492365674f995e4ad38c82a.torrent'),
  ('503f498e654cd54d8943234e1062b9cbc755b0d9', 'https://mikanani.me/Download/20260415/503f498e654cd54d8943234e1062b9cbc755b0d9.torrent'),
  ('f1dfd8089454214751d35e39b81da94a0bd4c99d', 'https://mikanani.me/Download/20260415/f1dfd8089454214751d35e39b81da94a0bd4c99d.torrent'),
  ('067cc2ffd8fb430cb0c26b8603d7932368df2e08', 'https://mikanani.me/Download/20260419/067cc2ffd8fb430cb0c26b8603d7932368df2e08.torrent'),
  ('52d31cc95aa37c44025edde857d07a677564ef98', 'https://mikanani.me/Download/20260419/52d31cc95aa37c44025edde857d07a677564ef98.torrent'),
  ('d9b4c158c06c423c4fd45d34d978aa3221d21a16', 'https://mikanani.me/Download/20260419/d9b4c158c06c423c4fd45d34d978aa3221d21a16.torrent'),
  ('bc02aba68cd3f86f5edbe3d2270409de244a625d', 'https://mikanani.me/Download/20260420/bc02aba68cd3f86f5edbe3d2270409de244a625d.torrent'),
  ('240ead43d9893763184dfbc17ac2a3cd2dfdbc29', 'https://mikanani.me/Download/20260421/240ead43d9893763184dfbc17ac2a3cd2dfdbc29.torrent'),
  ('4dccdf086d16f455f03749c858680c49597637f6', 'https://mikanani.me/Download/20260421/4dccdf086d16f455f03749c858680c49597637f6.torrent'),
  ('4938f57f18d7ee8d4b799de5d05e265c8f39087a', 'https://mikanani.me/Download/20260427/4938f57f18d7ee8d4b799de5d05e265c8f39087a.torrent'),
  ('8d57e7b775f5deeaf46dc716c7b9d2e3f912d27b', 'https://mikanani.me/Download/20260427/8d57e7b775f5deeaf46dc716c7b9d2e3f912d27b.torrent'),
  ('9ffc64637a6eaae0443dd4991bfc656404a64f84', 'https://mikanani.me/Download/20260427/9ffc64637a6eaae0443dd4991bfc656404a64f84.torrent'),
  ('ca120cc976f8e5657f9336a6fe3b75d773382ee7', 'https://mikanani.me/Download/20260427/ca120cc976f8e5657f9336a6fe3b75d773382ee7.torrent'),
  ('2c4839d87b0b90310fb1421996c9c466092aea08', 'https://mikanani.me/Download/20260501/2c4839d87b0b90310fb1421996c9c466092aea08.torrent'),
  ('5c54ba7546b9eed5e17ca924e3cd97ca73715506', 'https://mikanani.me/Download/20260501/5c54ba7546b9eed5e17ca924e3cd97ca73715506.torrent'),
  ('217d4c5ee9f8bacde6d7bc9341a202bfce627aef', 'https://mikanani.me/Download/20260504/217d4c5ee9f8bacde6d7bc9341a202bfce627aef.torrent'),
  ('485b70923b74bf4bbc5aaefb8db1b55d6b2ce395', 'https://mikanani.me/Download/20260504/485b70923b74bf4bbc5aaefb8db1b55d6b2ce395.torrent'),
  ('899a70eef4b95472d26ec1715e30303afa2b1d7e', 'https://mikanani.me/Download/20260504/899a70eef4b95472d26ec1715e30303afa2b1d7e.torrent'),
  ('dbf31dea965a37eb41001ba5482c6dd17cd24982', 'https://mikanani.me/Download/20260504/dbf31dea965a37eb41001ba5482c6dd17cd24982.torrent'),
  ('95a78f2dcf43e863638092675e87d859cd9721b6', 'https://mikanani.me/Download/20260505/95a78f2dcf43e863638092675e87d859cd9721b6.torrent'),
  ('e1084eb986089eeac8b4bb8e760432a300c17030', 'https://mikanani.me/Download/20260505/e1084eb986089eeac8b4bb8e760432a300c17030.torrent'),
  ('4ce94b2c9f14d95845e574216d2e548f118dfe63', 'https://mikanani.me/Download/20260506/4ce94b2c9f14d95845e574216d2e548f118dfe63.torrent'),
  ('9690c5bf2c5e303cbb1aa9e4e47e2ac4d62ad1bc', 'https://mikanani.me/Download/20260506/9690c5bf2c5e303cbb1aa9e4e47e2ac4d62ad1bc.torrent'),
  ('8d2f4d203f5331b0bec977011e3a8fc742f79db6', 'https://mikanani.me/Download/20260507/8d2f4d203f5331b0bec977011e3a8fc742f79db6.torrent'),
  ('b26fd720bf79cf1f65070600f2c6dc9aa8da9114', 'https://mikanani.me/Download/20260507/b26fd720bf79cf1f65070600f2c6dc9aa8da9114.torrent'),
  ('3b3d86b917a2edba6dc4c459a1c350d4eaf2f371', 'https://mikanani.me/Download/20260510/3b3d86b917a2edba6dc4c459a1c350d4eaf2f371.torrent'),
  ('4f2640ae676ce89a9c9435fa2e8fa8d3175dfcfe', 'https://mikanani.me/Download/20260510/4f2640ae676ce89a9c9435fa2e8fa8d3175dfcfe.torrent'),
  ('88def71c9e594e3d0659179aa98f73d69e5941d7', 'https://mikanani.me/Download/20260510/88def71c9e594e3d0659179aa98f73d69e5941d7.torrent'),
  ('f4e2ae368c8a32bd6abe620745552f38e585b5c1', 'https://mikanani.me/Download/20260511/f4e2ae368c8a32bd6abe620745552f38e585b5c1.torrent'),
  ('2f88caacf5c64e6ab7221dd6b012de4fc40da691', 'https://mikanani.me/Download/20260513/2f88caacf5c64e6ab7221dd6b012de4fc40da691.torrent'),
  ('546d74af9218bdd7a767cba9d1bbf63585fca9d1', 'https://mikanani.me/Download/20260513/546d74af9218bdd7a767cba9d1bbf63585fca9d1.torrent'),
  ('0e51152415040653948fd75ad5b1fdad751de70d', 'https://mikanani.me/Download/20260514/0e51152415040653948fd75ad5b1fdad751de70d.torrent'),
  ('e6e39a214027638237ff90473facdab93b8c18c5', 'https://mikanani.me/Download/20260514/e6e39a214027638237ff90473facdab93b8c18c5.torrent'),
  ('11ca5a1f8b7d41149953a3b6db4812cf05f585c3', 'https://mikanani.me/Download/20260518/11ca5a1f8b7d41149953a3b6db4812cf05f585c3.torrent'),
  ('a726358b3ea9ae814098cb50817224b7ee908405', 'https://mikanani.me/Download/20260518/a726358b3ea9ae814098cb50817224b7ee908405.torrent'),
  ('f3c52177e05afd01de7f7d5357ebdbe0e7fdc495', 'https://mikanani.me/Download/20260518/f3c52177e05afd01de7f7d5357ebdbe0e7fdc495.torrent'),
  ('f62c1a5b4dbefca97b588a05c9363c1564bfaa1e', 'https://mikanani.me/Download/20260518/f62c1a5b4dbefca97b588a05c9363c1564bfaa1e.torrent'),
  ('3b126ce96811e1ad2c3729e123589051e9548360', 'https://mikanani.me/Download/20260521/3b126ce96811e1ad2c3729e123589051e9548360.torrent'),
  ('63292366c7c193e22db13dc962011c014941e698', 'https://mikanani.me/Download/20260521/63292366c7c193e22db13dc962011c014941e698.torrent'),
  ('9e89666b40f0514b8bc090762c0bdba3c905dd69', 'https://mikanani.me/Download/20260521/9e89666b40f0514b8bc090762c0bdba3c905dd69.torrent'),
  ('dde898aaf708a89f7987ebb1d709ee1935154d4c', 'https://mikanani.me/Download/20260521/dde898aaf708a89f7987ebb1d709ee1935154d4c.torrent'),
  ('0c915c8fdd2517adf5327c921014fe8b074b9d9d', 'https://mikanani.me/Download/20260525/0c915c8fdd2517adf5327c921014fe8b074b9d9d.torrent'),
  ('7043ddd2c5f531a50e848522725012cdb396478d', 'https://mikanani.me/Download/20260525/7043ddd2c5f531a50e848522725012cdb396478d.torrent'),
  ('74263429a34dfab333042607d6adaa6250ddbe5f', 'https://mikanani.me/Download/20260525/74263429a34dfab333042607d6adaa6250ddbe5f.torrent'),
  ('e9e8a32f4ae657c448315d449a9569957f05952e', 'https://mikanani.me/Download/20260525/e9e8a32f4ae657c448315d449a9569957f05952e.torrent'),
  ('97d863fa6e1c973554ddee84b501fe2710637c5c', 'https://mikanani.me/Download/20260526/97d863fa6e1c973554ddee84b501fe2710637c5c.torrent'),
  ('cefac91a2cf82f62ab0c6deebeb10797d6e80214', 'https://mikanani.me/Download/20260526/cefac91a2cf82f62ab0c6deebeb10797d6e80214.torrent'),
  ('5636698369a128e26883876dc95dd8370cae5e1f', 'https://mikanani.me/Download/20260528/5636698369a128e26883876dc95dd8370cae5e1f.torrent'),
  ('6beda2c0375a780e7d49167b2f51a6d2e52c13c2', 'https://mikanani.me/Download/20260528/6beda2c0375a780e7d49167b2f51a6d2e52c13c2.torrent'),
  ('fa96aa1fd8b5e1bb077c18db6bc2233686126b8d', 'https://mikanani.me/Download/20260601/fa96aa1fd8b5e1bb077c18db6bc2233686126b8d.torrent'),
  ('23a18d61816ea2d42ff8b886004de3a1cadae3b0', 'https://mikanani.me/Download/20260602/23a18d61816ea2d42ff8b886004de3a1cadae3b0.torrent'),
  ('329f28aac039eb15f44abb0218705e94d5d8ca93', 'https://mikanani.me/Download/20260602/329f28aac039eb15f44abb0218705e94d5d8ca93.torrent'),
  ('5a2145e0bea5d22720984f658fcfef2eaaa677d7', 'https://mikanani.me/Download/20260602/5a2145e0bea5d22720984f658fcfef2eaaa677d7.torrent'),
  ('775ae0e9b7c9d393a1c8b9448d616754b2c2d733', 'https://mikanani.me/Download/20260602/775ae0e9b7c9d393a1c8b9448d616754b2c2d733.torrent'),
  ('be53d614f7d0d5bfdc7383a561788311d30ccd67', 'https://mikanani.me/Download/20260602/be53d614f7d0d5bfdc7383a561788311d30ccd67.torrent'),
  ('630721d9fac706e8a4cd57ace2580b7424a553f8', 'https://mikanani.me/Download/20260603/630721d9fac706e8a4cd57ace2580b7424a553f8.torrent'),
  ('70bbabb5e10f58cdd74615f4e24eba60fd2dc649', 'https://mikanani.me/Download/20260603/70bbabb5e10f58cdd74615f4e24eba60fd2dc649.torrent'),
  ('1c3ff4eac8222a4a2594750973849a46758cf825', 'https://mikanani.me/Download/20260607/1c3ff4eac8222a4a2594750973849a46758cf825.torrent'),
  ('56bb371535f9edd47b2c900e9095e7107b12114e', 'https://mikanani.me/Download/20260607/56bb371535f9edd47b2c900e9095e7107b12114e.torrent'),
  ('782d0b0211418b1adbee42fdb71b256e6df0f4c5', 'https://mikanani.me/Download/20260607/782d0b0211418b1adbee42fdb71b256e6df0f4c5.torrent'),
  ('36a998df1a158c8f40ae72af497b44f1dae10e73', 'https://mikanani.me/Download/20260608/36a998df1a158c8f40ae72af497b44f1dae10e73.torrent'),
  ('3ffb0c65507a8430014b0ae3245b157aa5cab5a8', 'https://mikanani.me/Download/20260610/3ffb0c65507a8430014b0ae3245b157aa5cab5a8.torrent'),
  ('fa0d52a6dcd34c9748d643a3918435fa4bab4009', 'https://mikanani.me/Download/20260610/fa0d52a6dcd34c9748d643a3918435fa4bab4009.torrent'),
  ('4140aeabc5b3b8e76f8a3c0f757e7c6f350ff7e1', 'https://mikanani.me/Download/20260613/4140aeabc5b3b8e76f8a3c0f757e7c6f350ff7e1.torrent'),
  ('91936ed0d7502eec6a2a7368d1d12ef2318967ab', 'https://mikanani.me/Download/20260613/91936ed0d7502eec6a2a7368d1d12ef2318967ab.torrent'),
  ('be8eab4c2f0e226a29afef0152c8adc5ae83e964', 'https://mikanani.me/Download/20260614/be8eab4c2f0e226a29afef0152c8adc5ae83e964.torrent'),
  ('d128f545a9146b4158aca228539c4d2eef9399aa', 'https://mikanani.me/Download/20260614/d128f545a9146b4158aca228539c4d2eef9399aa.torrent'),
  ('e58d0662718d41420c678635e05dfd0c0a8c4660', 'https://mikanani.me/Download/20260614/e58d0662718d41420c678635e05dfd0c0a8c4660.torrent'),
  ('c3b397e02692f484354f825b9257682a8a23da7b', 'https://mikanani.me/Download/20260615/c3b397e02692f484354f825b9257682a8a23da7b.torrent'),
  ('1245f42bfa1e3b23535fc95b5b9a5c875e585070', 'https://mikanani.me/Download/20260616/1245f42bfa1e3b23535fc95b5b9a5c875e585070.torrent'),
  ('43b3a5cb3085acbc85868bcc63e89138ff166115', 'https://mikanani.me/Download/20260616/43b3a5cb3085acbc85868bcc63e89138ff166115.torrent'),
  ('0f72be2da397adb05981fd8682a8ea1125547eeb', 'https://mikanani.me/Download/20260622/0f72be2da397adb05981fd8682a8ea1125547eeb.torrent'),
  ('92a5ee287151ad825fa19aba0be2a8cac4a1f268', 'https://mikanani.me/Download/20260622/92a5ee287151ad825fa19aba0be2a8cac4a1f268.torrent'),
  ('d1e4ac4b6663d18ca5ebb0014561596f4131384d', 'https://mikanani.me/Download/20260622/d1e4ac4b6663d18ca5ebb0014561596f4131384d.torrent'),
  ('ef5a27eecd3dbb3f081ca13043590762e53c5032', 'https://mikanani.me/Download/20260622/ef5a27eecd3dbb3f081ca13043590762e53c5032.torrent'),
  ('589f74cbc26e96cbefdd5cf2c5c904fd0177cf1d', 'https://mikanani.me/Download/20260623/589f74cbc26e96cbefdd5cf2c5c904fd0177cf1d.torrent'),
  ('6fe3f8ea2a37902ff92c715df4341d9cc3e3d58e', 'https://mikanani.me/Download/20260623/6fe3f8ea2a37902ff92c715df4341d9cc3e3d58e.torrent'),
  ('4d14dc648b16bb55a780491ace14c1576fddbc31', 'https://mikanani.me/Download/20260626/4d14dc648b16bb55a780491ace14c1576fddbc31.torrent'),
  ('b848f5c61b622efb8612de7f1a3c6fe5605a208b', 'https://mikanani.me/Download/20260626/b848f5c61b622efb8612de7f1a3c6fe5605a208b.torrent'),
  ('4996ff980bea97d21b96b309ea43e6f6e3b355af', 'https://mikanani.me/Download/20260629/4996ff980bea97d21b96b309ea43e6f6e3b355af.torrent'),
  ('7de45cc84faaa64a4a3d827a22eb006f51ced5fa', 'https://mikanani.me/Download/20260629/7de45cc84faaa64a4a3d827a22eb006f51ced5fa.torrent'),
  ('b0490ced1e0771f4085ea0ddeb19f1f14e2ed39d', 'https://mikanani.me/Download/20260629/b0490ced1e0771f4085ea0ddeb19f1f14e2ed39d.torrent'),
  ('404a6c47d941aae7fa9a3a1a2a83dfbc60e2d0eb', 'https://mikanani.me/Download/20260701/404a6c47d941aae7fa9a3a1a2a83dfbc60e2d0eb.torrent'),
  ('58de7514a40eb03241ce795e57707a230b54dbcb', 'https://mikanani.me/Download/20260701/58de7514a40eb03241ce795e57707a230b54dbcb.torrent'),
  ('3fc6ba4b3580043d97a10c7ec8342b089312ce9b', 'https://mikanani.me/Download/20260706/3fc6ba4b3580043d97a10c7ec8342b089312ce9b.torrent'),
  ('5bd993de0c78143ad8d9baae0e6562a16488cfef', 'https://mikanani.me/Download/20260706/5bd993de0c78143ad8d9baae0e6562a16488cfef.torrent'),
  ('68fa16ccd5454b1683c1232870f7fafcd89bb7e3', 'https://mikanani.me/Download/20260706/68fa16ccd5454b1683c1232870f7fafcd89bb7e3.torrent'),
  ('217e6da069a8ee782fb6d2cabe8e438f6293780c', 'https://mikanani.me/Download/20260708/217e6da069a8ee782fb6d2cabe8e438f6293780c.torrent'),
  ('bb9e4edf912d1d73178b5dbe18e69ce04051240c', 'https://mikanani.me/Download/20260708/bb9e4edf912d1d73178b5dbe18e69ce04051240c.torrent');

-- 1. raw_anime_items：改寫 download_url（去重鍵，之後 RSS 重抓會精確匹配）
UPDATE raw_anime_items r
SET download_url = m.new_url
FROM magnet_url_fix m
WHERE r.download_url LIKE 'magnet:%'
  AND substring(r.download_url from 'btih:([0-9a-f]+)') = m.hash;

-- 2. anime_links：改寫 url、download_type，並重算 source_hash = sha256(url)
--    （保留批次集數的 '#epN' 後綴；reparse 依賴 source_hash 與 url 的對應）
UPDATE anime_links al
SET url = m.new_url,
    download_type = 'torrent',
    source_hash = encode(digest(m.new_url, 'sha256'), 'hex')
                  || COALESCE(substring(al.source_hash from '#ep[0-9]+$'), '')
FROM magnet_url_fix m
WHERE al.url LIKE 'magnet:%'
  AND substring(al.url from 'btih:([0-9a-f]+)') = m.hash;

-- 3. downloads：去除重複派送產生的重複記錄（每個 link 保留最早一筆）
DELETE FROM downloads d
USING downloads keep
WHERE d.link_id = keep.link_id
  AND d.status = 'downloading'
  AND keep.status = 'downloading'
  AND keep.download_id < d.download_id;

-- 4. downloads：把卡在 0% 的 downloading 重置為 failed，
--    downloader 註冊時 retry_failed_downloads() 會以改寫後的 .torrent URL 重新派送
UPDATE downloads
SET status = 'failed',
    error_message = 'reset by 2026-07-14 magnet-url migration (dead trackers, no peers)',
    updated_at = NOW()
WHERE status = 'downloading'
  AND COALESCE(progress, 0) = 0;

-- 驗證輸出
SELECT 'raw_items magnet remaining' AS check_item, COUNT(*)::text AS value
  FROM raw_anime_items WHERE download_url LIKE 'magnet:%'
UNION ALL
SELECT 'anime_links magnet remaining', COUNT(*)::text
  FROM anime_links WHERE url LIKE 'magnet:%'
UNION ALL
SELECT 'downloads by status', status || '=' || COUNT(*)::text
  FROM downloads GROUP BY status;

COMMIT;
