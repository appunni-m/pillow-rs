/**
 * Strict loader for the shared version-2 Pillow oracle corpus.
 *
 * This module owns fixture discovery and structural validation for JavaScript
 * consumers. It does not execute pillow-rs or weaken oracle assertions.
 */

import {
    existsSync,
    readFileSync,
    readdirSync,
} from 'node:fs';
import { dirname, isAbsolute, join, normalize, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..');

export const PILLOW_VERSION = '12.2.0';
export const FREETYPE_VERSION = '2.14.3';

const suites = [
    { name: 'fixtures', root: join(repoRoot, 'tests', 'fixtures') },
    { name: 'fixtures_2', root: join(repoRoot, 'tests', 'fixtures_2') },
];

const assertionMethods = new Set([
    'error',
    'image',
    'image_list',
    'tuple',
    'typed',
    'typed_binary',
]);

const forbiddenAssertionFields = new Set([
    'message_contains',
    'prefix',
    'tolerance',
]);

function parseJson(path) {
    return JSON.parse(readFileSync(path, 'utf8'));
}

function jsonEqual(left, right) {
    return JSON.stringify(left) === JSON.stringify(right);
}

function uniqueIds(cases) {
    const ids = cases.map((item) => item.id);
    return new Set(ids).size === ids.length;
}

function safeReference(root, reference) {
    if (typeof reference !== 'string' || reference.length === 0) return false;
    if (isAbsolute(reference)) return false;
    const resolved = normalize(join(root, reference));
    const rel = relative(root, resolved);
    return rel !== '..' && !rel.startsWith(`..${process.platform === 'win32' ? '\\' : '/'}`);
}

function validateAssertion(assertion, context, outputRoot, errors) {
    if (!assertion || typeof assertion !== 'object' || Array.isArray(assertion)) {
        errors.push(`${context}: assertion is not an object`);
        return;
    }

    const method = assertion.method;
    if (!assertionMethods.has(method)) {
        errors.push(`${context}: unsupported assertion method ${JSON.stringify(method)}`);
        return;
    }

    for (const field of forbiddenAssertionFields) {
        if (Object.hasOwn(assertion, field)) {
            errors.push(`${context}: forbidden non-exact assertion field ${field}`);
        }
    }

    if (method === 'error') {
        if (typeof assertion.exception !== 'string') {
            errors.push(`${context}: error assertion lacks exact exception type`);
        }
        if (typeof assertion.message !== 'string') {
            errors.push(`${context}: error assertion lacks exact message`);
        }
    }

    if (method === 'image' || method === 'typed_binary') {
        const reference = assertion.reference;
        if (!safeReference(outputRoot, reference)) {
            errors.push(`${context}: unsafe image reference ${JSON.stringify(reference)}`);
        } else if (!existsSync(join(outputRoot, reference))) {
            errors.push(`${context}: missing image reference ${reference}`);
        }
        if (method === 'image' && reference?.endsWith('.bin')) {
            if (typeof assertion.raw_kind !== 'string') {
                errors.push(`${context}: raw image assertion lacks raw_kind`);
            }
            if (
                assertion.raw_kind === 'image'
                && (!Array.isArray(assertion.size) || typeof assertion.mode !== 'string')
            ) {
                errors.push(`${context}: raw image assertion lacks exact mode or size`);
            }
        }
    }

    if (method === 'image_list') {
        if (!Array.isArray(assertion.items)) {
            errors.push(`${context}: image_list assertion lacks items`);
        }
        if (typeof assertion.container_type !== 'string') {
            errors.push(`${context}: image_list assertion lacks container_type`);
        }
    }

    if (method === 'tuple' && !Array.isArray(assertion.items)) {
        errors.push(`${context}: tuple assertion lacks items`);
    }

    if (method === 'typed' && !Object.hasOwn(assertion, 'value')) {
        errors.push(`${context}: typed assertion lacks value`);
    }

    for (const [index, item] of (assertion.items ?? []).entries()) {
        validateAssertion(item, `${context}/item-${index}`, outputRoot, errors);
    }
}

/**
 * Load every canonical fixture case or throw one aggregate contract error.
 *
 * @returns {{operations: number, cases: Array<object>, suites: Array<object>}}
 */
export function loadOracleCorpus() {
    const errors = [];
    const discoveredCases = [];
    const suiteCounts = [];
    let operationCount = 0;
    const globalCaseKeys = new Set();

    for (const suite of suites) {
        const inputJsonDir = join(suite.root, 'input', 'jsons');
        const outputJsonDir = join(suite.root, 'outputs', 'jsons');
        const outputRoot = join(suite.root, 'outputs');
        const inputNames = new Set(
            readdirSync(inputJsonDir).filter((name) => name.endsWith('.json')),
        );
        const outputNames = new Set(
            readdirSync(outputJsonDir).filter((name) => name.endsWith('.json')),
        );

        for (const name of inputNames) {
            if (!outputNames.has(name)) {
                errors.push(`${suite.name}: missing oracle output ${name}`);
            }
        }
        for (const name of outputNames) {
            if (!inputNames.has(name)) {
                errors.push(`${suite.name}: orphan oracle output ${name}`);
            }
        }

        let suiteCaseCount = 0;
        for (const name of [...inputNames].filter((item) => outputNames.has(item)).sort()) {
            const inputPath = join(inputJsonDir, name);
            const outputPath = join(outputJsonDir, name);
            const input = parseJson(inputPath);
            const output = parseJson(outputPath);
            operationCount += 1;

            if (input.format_version !== 2) {
                errors.push(`${suite.name}/${name}: input format_version is not 2`);
            }
            if (output.format_version !== 2) {
                errors.push(`${suite.name}/${name}: output format_version is not 2`);
            }
            if (output.pillow_version !== PILLOW_VERSION) {
                errors.push(
                    `${suite.name}/${name}: Pillow ${JSON.stringify(output.pillow_version)}`
                    + ` != ${JSON.stringify(PILLOW_VERSION)}`,
                );
            }
            if (output.freetype_version !== FREETYPE_VERSION) {
                errors.push(
                    `${suite.name}/${name}: FreeType ${JSON.stringify(output.freetype_version)}`
                    + ` != ${JSON.stringify(FREETYPE_VERSION)}`,
                );
            }
            if (!jsonEqual(input.operation, output.operation)) {
                errors.push(`${suite.name}/${name}: input/output operation differs`);
            }
            if ((input.suite ?? 0) !== (output.suite ?? 0)) {
                errors.push(`${suite.name}/${name}: input/output suite differs`);
            }
            if (!uniqueIds(input.cases ?? [])) {
                errors.push(`${suite.name}/${name}: duplicate input case IDs`);
            }
            if (!uniqueIds(output.cases ?? [])) {
                errors.push(`${suite.name}/${name}: duplicate output case IDs`);
            }

            const outputCases = new Map(
                (output.cases ?? []).map((item) => [item.id, item]),
            );
            const inputIds = new Set((input.cases ?? []).map((item) => item.id));
            for (const inputCase of input.cases ?? []) {
                const outputCase = outputCases.get(inputCase.id);
                if (!outputCase) {
                    errors.push(
                        `${suite.name}/${name}/${inputCase.id}: missing oracle case`,
                    );
                    continue;
                }

                const caseKey = `${name}__${inputCase.id}`;
                if (globalCaseKeys.has(caseKey)) {
                    errors.push(`duplicate cross-suite case key ${caseKey}`);
                }
                globalCaseKeys.add(caseKey);
                validateAssertion(
                    outputCase.assert,
                    `${suite.name}/${name}/${inputCase.id}`,
                    outputRoot,
                    errors,
                );
                discoveredCases.push({
                    id: caseKey,
                    suite: suite.name,
                    fixture: name,
                    operation: input.operation,
                    input: inputCase,
                    assertion: outputCase.assert,
                    inputRoot: join(suite.root, 'input'),
                    outputRoot,
                });
                suiteCaseCount += 1;
            }
            for (const outputCase of output.cases ?? []) {
                if (!inputIds.has(outputCase.id)) {
                    errors.push(
                        `${suite.name}/${name}/${outputCase.id}: orphan oracle case`,
                    );
                }
            }
        }
        suiteCounts.push({
            name: suite.name,
            operations: [...inputNames].filter((item) => outputNames.has(item)).length,
            cases: suiteCaseCount,
        });
    }

    if (errors.length > 0) {
        throw new Error(
            `${errors.length} Pillow oracle contract error(s):\n${errors.join('\n')}`,
        );
    }

    return {
        operations: operationCount,
        cases: discoveredCases,
        suites: suiteCounts,
    };
}
