const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const fs = require('node:fs/promises');
const { createServer } = require('node:http');
const path = require('node:path');
const { chromium } = require('playwright');

async function main() {
    assert(process.env.PWT_TEST_CSS, 'Set PWT_TEST_CSS to a compiled PWT theme');
    const css = path.resolve(process.env.PWT_TEST_CSS);
    const assets = path.resolve(process.env.PWT_TEST_ASSETS || path.dirname(css));
    const output = path.join(__dirname, 'build', new Date().toISOString().replaceAll(':', '-'));
    const dist = path.join(output, 'dist');
    await fs.mkdir(output, { recursive: true });
    await fs.copyFile(css, path.join(__dirname, 'theme.css'));
    const icons = (await fs.readdir(assets)).find((name) =>
        /^font-awesome(?:[-.].*)?\.css$/.test(name),
    );
    assert(icons, 'PWT_TEST_ASSETS must contain font-awesome.css and its fonts directory');
    await fs.copyFile(path.join(assets, icons), path.join(__dirname, 'font-awesome.css'));
    const build = spawnSync('trunk', ['build', '--dist', dist], {
        cwd: __dirname,
        stdio: 'inherit',
    });
    assert.equal(build.status, 0, 'trunk build failed');
    const server = createServer(async (request, response) => {
        const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
        const relative = pathname === '/' ? 'index.html' : pathname.slice(1);
        for (const root of [dist, assets]) {
            const file = path.resolve(root, relative);
            if (!file.startsWith(`${root}${path.sep}`)) continue;
            try {
                const body = await fs.readFile(file);
                const types = {
                    '.wasm': 'application/wasm',
                    '.js': 'text/javascript',
                    '.css': 'text/css',
                    '.woff2': 'font/woff2',
                };
                response.writeHead(200, {
                    'Content-Type': types[path.extname(file)] || 'text/html',
                });
                response.end(body);
                return;
            } catch {}
        }
        response.writeHead(404);
        response.end();
    });
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const browser = await chromium
        .launch({
            executablePath: process.env.CHROMIUM || '/usr/bin/chromium',
            headless: true,
        })
        .catch((error) => {
            server.close();
            throw error;
        });
    let cases = 0;
    async function test(
        name,
        query,
        width,
        callback,
        touch = false,
        reducedMotion = 'no-preference',
    ) {
        if (process.env.PWT_TEST_FILTER && !name.includes(process.env.PWT_TEST_FILTER)) return;
        const page = await browser.newPage({
            viewport: { width, height: 844 },
            hasTouch: touch,
            reducedMotion,
        });
        const errors = [];
        page.on('pageerror', (error) => errors.push(error.stack));
        try {
            await page.goto(`http://127.0.0.1:${server.address().port}/?${query}`);
            await page.locator('dialog:modal').waitFor();
            await page.waitForTimeout(300);
            await page.evaluate(() => document.fonts.ready);
            assert(await page.evaluate(() => document.fonts.check('16px FontAwesome')));
            await callback(page);
            assert.deepEqual(errors, [], 'browser errors');
            cases++;
            console.log(`ok ${name}`);
        } catch (error) {
            await page.screenshot({ path: path.join(output, `failed-${name}.png`) });
            throw error;
        } finally {
            await page.close();
        }
    }
    const events = async (page) =>
        (await page.locator('#events').innerText()).trim().split('\n').filter(Boolean);
    async function fill(page) {
        await page.locator('input').first().fill('entered value');
        await page
            .locator('input')
            .first()
            .evaluate((element) => {
                element.dataset.retained = 'yes';
            });
    }
    async function retained(page) {
        assert.equal(await page.locator('input').first().inputValue(), 'entered value');
        assert.equal(await page.locator('input').first().getAttribute('data-retained'), 'yes');
        assert.equal(await page.locator('dialog:modal').count(), 1);
        assert.equal(
            await page.locator('#outside').evaluate((element) => {
                element.focus();
                return document.activeElement === element;
            }),
            false,
        );
    }
    async function close(page, method) {
        if (method === 'Escape') {
            await page.keyboard.press('Escape');
        } else {
            const button = page
                .locator('dialog:modal')
                .last()
                .getByRole('button', { name: 'Close', exact: true });
            if (method === 'click') await button.click();
            else {
                await button.focus();
                await page.keyboard.press(method);
            }
        }
        await page.waitForTimeout(300);
    }
    function fits(rect, width) {
        assert(
            rect.x >= 0 &&
                rect.y >= 0 &&
                rect.x + rect.width <= width + 1 &&
                rect.y + rect.height <= 845,
            JSON.stringify(rect),
        );
    }
    try {
        for (const [layout, width] of [
            ['plain', 1100],
            ['adaptive-wide', 1100],
            ['adaptive-narrow', 390],
        ]) {
            for (const method of ['click', 'Enter', 'Space', 'Escape']) {
                for (const mode of ['deny', 'accept', 'unguarded']) {
                    await test(
                        `${layout}-${mode}-${method}`,
                        `${layout}&${mode}`,
                        width,
                        async (page) => {
                            await fill(page);
                            await close(page, method);
                            if (mode === 'deny') await retained(page);
                            else assert.equal(await page.locator('dialog').count(), 0);
                            assert.deepEqual(await events(page), [
                                ...(mode === 'unguarded' ? [] : ['check:entered value']),
                                ...(mode === 'deny' ? [] : ['close', 'done']),
                            ]);
                        },
                    );
                }
            }
            await test(`${layout}-submit`, layout, width, async (page) => {
                await fill(page);
                await page.getByRole('button', { name: 'Save', exact: true }).click();
                await page.waitForTimeout(100);
                assert.deepEqual(await events(page), ['submit', 'done']);
                assert.equal(await page.locator('dialog').count(), 0);
            });
            await test(`${layout}-failure-layout`, `${layout}&failure`, width, async (page) => {
                await fill(page);
                const save = page.getByRole('button', { name: 'Save', exact: true });
                await save.click();
                await page.getByText(/^Save failed\./).waitFor();
                fits(await save.boundingBox(), width);
                assert(
                    await save.evaluate((element) => {
                        const r = element.getBoundingClientRect();
                        return element.contains(
                            document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2),
                        );
                    }),
                    'save action is clipped or covered',
                );
                await page.screenshot({ path: path.join(output, `${layout}-inline-error.png`) });
                await close(page, 'Escape');
                await retained(page);
                assert.deepEqual(await events(page), ['submit', 'check:entered value']);
            });
            await test(`${layout}-confirmation`, `${layout}&confirm`, width, async (page) => {
                await fill(page);
                for (const cancel of ['Keep editing', 'Escape', 'click']) {
                    await close(page, 'click');
                    assert.equal(await page.locator('dialog:modal').count(), 2);
                    if (cancel === 'Keep editing')
                        await page.getByRole('button', { name: cancel, exact: true }).click();
                    else await close(page, cancel);
                    await retained(page);
                }
                assert.deepEqual(await events(page), Array(3).fill('check:entered value'));
                await close(page, 'click');
                await page.getByRole('button', { name: 'Discard', exact: true }).click();
                assert.equal(await page.locator('dialog').count(), 0);
            });
        }
        for (const touch of [false, true]) {
            await test(
                `swipe-${touch}`,
                'adaptive&confirm',
                390,
                async (page) => {
                    await fill(page);
                    for (let attempt = 0; attempt < 2; attempt++) {
                        await page.waitForTimeout(300);
                        const rect = await page.locator('.pwt-side-dialog-slider').boundingBox();
                        const x = 195;
                        const y = rect.y + 8;
                        if (touch) {
                            const cdp = await page.context().newCDPSession(page);
                            await cdp.send('Input.dispatchTouchEvent', {
                                type: 'touchStart',
                                touchPoints: [{ x, y }],
                            });
                            for (const delta of [30, 60, 90, 120, 150, 180]) {
                                await cdp.send('Input.dispatchTouchEvent', {
                                    type: 'touchMove',
                                    touchPoints: [{ x, y: y + delta }],
                                });
                            }
                            await cdp.send('Input.dispatchTouchEvent', {
                                type: 'touchEnd',
                                touchPoints: [],
                            });
                            await cdp.detach();
                        } else {
                            await page.mouse.move(x, y);
                            await page.mouse.down();
                            await page.mouse.move(x, y + 180, { steps: 6 });
                            await page.mouse.up();
                        }
                        await page.waitForTimeout(300);
                        assert.deepEqual(
                            await events(page),
                            Array(attempt + 1).fill('check:entered value'),
                        );
                        assert.equal(await page.locator('dialog:modal').count(), 2);
                        await page
                            .getByRole('button', { name: 'Keep editing', exact: true })
                            .click();
                        await retained(page);
                    }
                },
                touch,
            );
        }
        await test('backdrop', 'adaptive', 390, async (page) => {
            await fill(page);
            await page.mouse.click(5, 5);
            await page.waitForTimeout(100);
            await retained(page);
            assert.deepEqual(await events(page), ['check:entered value']);
        });
        await test(
            'reduced-motion-close',
            'adaptive&accept',
            390,
            async (page) => {
                await close(page, 'Escape');
                assert.equal(await page.locator('dialog').count(), 0);
            },
            false,
            'reduce',
        );
        for (const width of [1100, 390]) {
            for (const method of ['click', 'Enter', 'Space', 'Escape']) {
                await test(`wizard-pristine-${width}-${method}`, 'wizard', width, async (page) => {
                    await close(page, method);
                    assert.deepEqual(await events(page), ['check:', 'close', 'done']);
                    assert.equal(await page.locator('dialog').count(), 0);
                });
                await test(`wizard-confirm-${width}-${method}`, 'wizard', width, async (page) => {
                    await fill(page);
                    await close(page, method);
                    assert.equal(await page.locator('dialog:modal').count(), 2);
                    await page.getByRole('button', { name: 'Keep editing', exact: true }).click();
                    await retained(page);
                    assert.deepEqual(await events(page), ['check:entered value']);
                    await close(page, method);
                    await page.getByRole('button', { name: 'Discard', exact: true }).click();
                    assert.equal(await page.locator('dialog').count(), 0);
                });
            }
            await test(`wizard-pages-${width}`, 'wizard', width, async (page) => {
                await fill(page);
                await page.getByRole('button', { name: 'Next', exact: true }).click();
                await page.locator('input:visible').fill('details retained');
                await close(page, 'Escape');
                assert.equal(await page.locator('dialog:modal').count(), 2);
                await close(page, 'Escape');
                assert.equal(await page.locator('input:visible').inputValue(), 'details retained');
                await page.getByRole('button', { name: 'Back', exact: true }).click();
                await retained(page);
                await page.getByRole('button', { name: 'Next', exact: true }).click();
                await page.screenshot({ path: path.join(output, `wizard-${width}.png`) });
                await page.getByRole('button', { name: 'Finish', exact: true }).click();
                await page.waitForTimeout(100);
                assert.deepEqual(await events(page), ['check:entered value', 'submit', 'done']);
                assert.equal(await page.locator('dialog').count(), 0);
            });
        }
        await test('growth', 'growth', 1100, async (page) => {
            const initial = await page.locator('dialog').boundingBox();
            await page.getByRole('button', { name: 'Grow', exact: true }).click();
            await page.waitForTimeout(100);
            const grown = await page.locator('dialog').boundingBox();
            fits(grown, 1100);
            assert(grown.y < initial.y);
            await page.getByRole('button', { name: 'Shrink', exact: true }).click();
            await page.waitForTimeout(100);
            assert.equal((await page.locator('dialog').boundingBox()).y, grown.y);
        });
        for (const operation of ['drag', 'resize']) {
            await test(`growth-during-${operation}`, 'growth', 1100, async (page) => {
                const selector =
                    operation === 'drag' ? '.pwt-draggable' : '.dialog-resize-handle.east';
                const handle = await page.locator(selector).boundingBox();
                const x = handle.x + handle.width / 2;
                const y = handle.y + (operation === 'drag' ? 2 : handle.height / 2);
                await page.mouse.move(x, y);
                await page.mouse.down();
                await page.mouse.move(x + 80, y + 80, { steps: 4 });
                const moving = await page.locator('dialog').boundingBox();
                await page
                    .getByRole('button', { name: 'Grow', exact: true })
                    .evaluate((el) => el.click());
                await page.waitForTimeout(100);
                const grown = await page.locator('dialog').boundingBox();
                assert.equal(grown.x, moving.x);
                assert.equal(grown.y, moving.y);
                await page.mouse.up();
                await page.waitForTimeout(100);
                fits(await page.locator('dialog').boundingBox(), 1100);
            });
        }
        assert(cases > 0, 'No browser cases matched PWT_TEST_FILTER');
        console.log(`${cases} browser cases passed; artifacts: ${output}`);
    } finally {
        await browser.close();
        await new Promise((resolve) => server.close(resolve));
    }
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
