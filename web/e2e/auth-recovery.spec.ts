import {test,expect} from './util-test';
import {url,expectClean} from './util';

test('пароль можно показать, восстановление и подтверждение почты доступны без входа',async({page,context},info)=>{
 test.setTimeout(60000);
 await context.clearCookies();
 await page.route('**/api/site',async route=>{const response=await route.fetch();await route.fulfill({json:{...await response.json(),emailRecovery:true}});});
 await page.goto(url('/login'));
 const password=page.getByLabel('Пароль',{exact:true});
 await password.fill('local-test-password');
 await page.getByRole('button',{name:'Показать пароль',exact:true}).click();
 await expect(password).toHaveAttribute('type','text');await expect(password).toHaveValue('local-test-password');
 await page.getByRole('button',{name:'Скрыть пароль',exact:true}).click();
 await expect(password).toHaveAttribute('type','password');
 await expectClean(page,'вход с показом пароля');
 await page.screenshot({path:`test-results/login-${info.project.name}.png`});
 await page.getByRole('link',{name:'Забыли пароль?'}).click();
 await expect(page.getByRole('heading',{name:'Восстановить пароль'})).toBeVisible();
 await expect(page.getByText('Почта не нужна.',{exact:false})).toBeVisible();
 await page.getByText('Восстановить через настроенную почту',{exact:true}).click();
 await page.route('**/api/auth/forgot-password',route=>route.fulfill({json:{ok:true,message:'Если эта почта подтверждена в Nova, на неё отправлена ссылка.'}}));
 await page.getByLabel('Электронная почта',{exact:true}).fill('viewer@example.com');
 await page.getByRole('button',{name:'Получить ссылку'}).click();
 await expect(page.locator('p[role=status]')).toContainText('на неё отправлена ссылка');
 await expectClean(page,'восстановление');
 await page.goto(url('/reset-password#token='+'a'.repeat(64)));
 await page.getByLabel('Новый пароль',{exact:true}).fill('NewPassword123');
 await page.getByLabel('Повторите пароль',{exact:true}).fill('different');
 await page.getByRole('button',{name:'Сохранить пароль'}).click();
 await expect(page.getByRole('alert')).toHaveText('Пароли не совпадают');
 await page.getByLabel('Повторите пароль',{exact:true}).fill('NewPassword123');
 await page.route('**/api/auth/reset-password',route=>route.fulfill({json:{ok:true}}));
 await page.getByRole('button',{name:'Сохранить пароль'}).click();
 await expect(page.getByRole('heading',{name:'Пароль изменён'})).toBeVisible();expect(new URL(page.url()).hash).toBe('');
 await expectClean(page,'пароль изменён');
 await page.route('**/api/auth/verify-email',route=>route.fulfill({json:{ok:true}}));
 await page.goto(url('/verify-email#token='+'b'.repeat(64)));
 await page.getByRole('button',{name:'Подтвердить адрес'}).click();
 await expect(page.getByRole('heading',{name:'Почта подтверждена'})).toBeVisible();
 await expectClean(page,'подтверждение почты');
});

test('Gmail и подтверждение почты: раскрытые настройки и аккаунт без наложений',async({page},info)=>{
 await page.goto(url('/admin?tab=settings'));
 const mail=page.getByRole('region',{name:'Почта и восстановление доступа'});
 await expect(mail).toBeVisible();
 await expect(page.getByLabel('SMTP-сервер',{exact:true})).toBeHidden();
 await page.getByText('Параметры SMTP и адрес Nova',{exact:true}).click();
 await expect(page.getByLabel('SMTP-сервер',{exact:true})).toBeVisible();
 // Шапка намеренно накрывает прокручиваемую страницу; проверяем всю геометрию из исходной позиции.
 await page.evaluate(()=>scrollTo(0,0));
 await expectClean(page,'настройки Gmail открыты');
 await mail.screenshot({path:`test-results/mail-${info.project.name}.png`});
 await page.goto(url('/profile?tab=account'));
 await expect(page.getByRole('heading',{name:'Почта для восстановления',exact:true})).toBeVisible();
 await expectClean(page,'почта и пароли в аккаунте');
 await page.screenshot({path:`test-results/account-email-${info.project.name}.png`,fullPage:true});
});
