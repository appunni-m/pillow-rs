import { loadOracleCorpus } from './oracle_corpus.mjs';

const corpus = loadOracleCorpus();

for (const suite of corpus.suites) {
    console.log(
        `${suite.name}: ${suite.operations} operations, ${suite.cases} cases`,
    );
}
console.log(
    `Pillow oracle contract: ${corpus.operations} operations,`
    + ` ${corpus.cases.length} exact cases`,
);
