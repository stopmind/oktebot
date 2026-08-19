# Установка

Для установки пропишите команду:
```
curl https://raw.githubusercontent.com/stopmind/oktebot/refs/heads/master/scripts/install | sh
```
После установки в системе появятся: сам бот `oktebot` и менеджер `oktebot-manage`.
За удаление отвечает команда `oktebot-manage remove`, а за обновление `oktebot-manage update`, для обеих нужны root права.

# Конфигурация

По умолчанию `oktebot` ищет конфиг по пути `/etc/oktebot.toml`, но это может быть переопределено переменной окружения `OKTEBOT_CONFIG`.
Конфигурация пишется в формате TOML.

Опции конфига:
- `token` - токен бота
- `support_chat` - id чата поддержки, куда будут перенаправляться сообщения, ВАЖНО: если это группа, то в начале должен быть добавлен минус. 
- `super_admins` - список id супер админов, по умолчанию пустой.
- `storage` - путь для хранения файлов ботом, по умолчанию `/var/oktebot`.