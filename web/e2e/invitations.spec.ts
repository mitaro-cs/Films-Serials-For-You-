import {expect,test} from './util-test';
import {url} from './util';

test('непрочитанное приглашение показывает уведомление со ссылкой',async({page})=>{
 await page.route('**/api/badges',route=>route.fulfill({json:{requests:0,unread:1,wishes:0,invitation:{id:987654,from:'Анна',room:'test-room'},watchedAt:0}}));
 await page.goto(url('/'));
 await expect(page.getByText('Анна приглашает смотреть вместе')).toBeVisible();
 await expect(page.getByRole('button',{name:'Открыть',exact:true})).toBeVisible();
 await page.getByRole('button',{name:'Открыть',exact:true}).click();
 await expect(page).toHaveURL(/\/room\/test-room/);
});
