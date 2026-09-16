"""Regressions for stale installations, release identity, and reader journeys."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from docs_release import check_source_version, check_documents, check_reader_page, get_json, newest_release, published, release_block, remote_release, render_blocks
from docs_site import check_html


def configuration():
    return {'project':'library', 'repository':'owner/library', 'version':'2.0.0-alpha.2',
            'release_revision':'a'*40, 'site_url':'https://owner.github.io/library/',
            'published_release':{'version':'1.0.0-alpha.1','revision':'a'*40,'published_at':'2026-09-16T00:00:00Z'},
            'registries':['cargo','npm','pypi'], 'pages':[{'source':'README.md','output':'index.md','audience':'user'}]}


class ReleaseDocumentationTests(unittest.TestCase):
    def test_source_version_drift_is_rejected_without_require_unpublished_install(self):
        config=configuration()
        check_source_version(config, '2.0.0-alpha.2')
        with self.assertRaisesRegex(ValueError, 'source version'):
            check_source_version(config, '3.0.0')

    def test_benchmark_results_precede_contributor_details_without_losing_rows(self):
        from docs_evidence import render_benchmarks
        repo=Path(__file__).resolve().parent.parent
        config=json.loads((repo/'documentation.json').read_text())
        snapshot=json.loads((repo/config['benchmark']['source']).read_text())
        with tempfile.TemporaryDirectory() as directory:
            output=Path(directory);(output/'assets').mkdir()
            text=render_benchmarks(repo,config,output)
            self.assertIn('class="bench-comparison"',text)
            self.assertNotIn('Source document SHA-256',text)
            self.assertNotIn('```json',text)
            self.assertIn(snapshot['source_sha256'],(output/'benchmark-details.md').read_text())
            self.assertEqual(json.loads((output/'assets/benchmark.json').read_text()),snapshot)
            # One comparative row per workload; every original observation is
            # retained in the downloadable snapshot and contributor table.
            self.assertEqual(text.count('class="bench-workload"'),len({row['workload'] for row in snapshot['rows']}))
            details=(output/'benchmark-details.md').read_text()
            for row in snapshot['rows']:
                self.assertIn(row['workload'],details)

    def test_latest_includes_prereleases_and_excludes_drafts(self):
        rows=[{'tag_name':'v0.9.0','published_at':'2026-09-01T00:00:00Z'},
              {'tag_name':'v1.0.0-alpha.1','prerelease':True,'published_at':'2026-09-16T00:00:00Z'},
              {'tag_name':'v2.0.0','draft':True,'published_at':'2026-10-01T00:00:00Z'}]
        self.assertEqual(newest_release(rows)['tag_name'], 'v1.0.0-alpha.1')

    def test_candidate_version_does_not_masquerade_as_published_version(self):
        config=configuration()
        self.assertNotEqual(config['version'], published(config)['version'])
        text=release_block('summary',config)+'\n'+release_block('cargo',config)
        self.assertEqual(check_reader_page(text,'README.md',config),[])
        stale=text.replace('=1.0.0-alpha.1','=2.0.0-alpha.2')
        self.assertTrue(check_reader_page(stale,'README.md',config))
        self.assertEqual(render_blocks(stale,config),text)

    def test_unmarked_stale_registry_and_api_pins_are_rejected(self):
        config=configuration(); summary=release_block('summary',config)
        for example in ['npm install library@0.9.0','pip install library==0.9.0',
                        'library = { version = "=0.9.0" }','https://docs.rs/library/0.9.0/library/']:
            with self.subTest(example=example):
                self.assertTrue(check_reader_page(summary+'\n'+example,'README.md',config))

    def test_malformed_markers_fail_instead_of_silently_skipping_a_command(self):
        for text in ['<!-- release:npm -->oops','<!-- release:npn -->x<!-- /release:npn -->',
                     '<!-- release:cargo -->x<!-- /release:npm -->']:
            with self.subTest(text=text), self.assertRaisesRegex(ValueError,'malformed'):
                render_blocks(text,configuration())

    def test_test_instructions_belong_only_to_contributors(self):
        config=configuration(); summary=release_block('summary',config)
        for command in ['make test', 'cargo test', 'npm run build', 'git clone example', 'tests/fixtures/input/font.ttf']:
            self.assertTrue(check_reader_page(summary+'\n```sh\n'+command+'\n```','README.md',config))
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); (root/'README.md').write_text(summary)
            (root/'CONTRIBUTING.md').write_text('make test\n')
            config['pages'].append({'source':'CONTRIBUTING.md','output':'contributing.md','audience':'contributor'})
            check_documents(root,config)
            config['pages'][0]['audience']='contributor'
            with self.assertRaisesRegex(ValueError,'README must remain'):
                check_documents(root,config)

    def test_tag_and_all_registries_are_checked_including_python_normalization(self):
        config=configuration(); calls=[]
        def fetch(url):
            calls.append(url)
            if '/releases?' in url:
                return [{'tag_name':'v1.0.0-alpha.1','published_at':config['published_release']['published_at']}]
            if '/git/ref/' in url: return {'object':{'type':'tag','sha':'b'*40}}
            if '/git/tags/' in url: return {'object':{'type':'commit','sha':'a'*40}}
            if 'crates.io' in url: return {'version':{'num':'1.0.0-alpha.1'}}
            if 'npmjs.org' in url: return {'version':'1.0.0-alpha.1'}
            if 'pypi.org' in url: return {'info':{'version':'1.0.0a1'}}
            self.fail(url)
        self.assertEqual(remote_release(config,fetch),config['published_release'])
        self.assertTrue(any('/1.0.0a1/json' in url for url in calls))
        def mismatch(url):
            return {'version':'0.9.0'} if 'npmjs.org' in url else fetch(url)
        with self.assertRaisesRegex(ValueError,'npm release version mismatch'):
            remote_release(config,mismatch)
        with self.assertRaises(OSError):
            remote_release(config,lambda url: (_ for _ in ()).throw(OSError('offline')))

    def test_github_credentials_never_go_to_registries(self):
        with patch.dict('os.environ',{'GH_TOKEN':'test-placeholder'}), patch('urllib.request.urlopen') as request:
            request.side_effect=OSError('stop before request')
            for url in ['https://registry.npmjs.org/library/1.0.0','https://api.github.com/repos/owner/library']:
                with self.assertRaises(OSError): get_json(url)
                headers=request.call_args.args[0].headers
                self.assertEqual('Authorization' in headers,url.startswith('https://api.github.com/'))

    def test_absolute_links_to_own_pages_are_validated(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); site=root/'target/site';site.mkdir(parents=True)
            (root/'documentation.json').write_text(json.dumps(configuration()))
            (site/'index.html').write_text('<a href="https://owner.github.io/library/guide/#missing">Guide</a>')
            (site/'guide').mkdir(); (site/'guide/index.html').write_text('<h1 id="guide">Guide</h1>')
            with self.assertRaisesRegex(ValueError,'missing rendered anchor'): check_html(root)
            (site/'index.html').write_text('<a href="https://owner.github.io/library/guide/#guide">Guide</a>')
            check_html(root)


if __name__ == '__main__':
    unittest.main()
