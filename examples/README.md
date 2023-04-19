# Конфигурация
Большинство примеров предполагают наличие библиотеки коннектора и аккаунта для подключения к серверу Transaq.

Создайте файл `.env` в корневой директории со следующим содержанием:

*libtxc/.env*
```
TXC_LOGIN = 'логин сервиса Transaq Connector'
TXC_PASSWORD = 'пароль'
TXC_LIB = 'путь к txmlconnector64.dll или txcn64.dll'
TXC_LOG_DIR = 'путь к директории для логов коннектора'
```

# Запуск
> cargo run --release --example EXAMPLE 

# Примеры
- [`offline`](offline.rs) - Демонстрaция базового использования библиотеки без подключения к серверу
- [`basic`](basic.rs) - Демонстрaция базового использования библиотеки, требует наличия аккаунта
- [`input_filter`](input_filter.rs) - Использование комбинаторов для фильтрации входящих сообщений
- [`threading`](threading.rs) - Пример многопоточного приложения 
- [`instrumentation`](instrumentation.rs) - Профилирования с использованием [`tracy`](https://github.com/wolfpld/tracy)
