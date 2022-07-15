# Predict V2

## 数据库

```sql
/* new databases */
CREATE DATABASE IF NOT EXISTS `xprotocol`;
USE `xprotocol`;

CREATE TABLE IF NOT EXISTS `banner` (
    `id` int unsigned NOT NULL AUTO_INCREMENT,
    `url` varchar(255) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
    PRIMARY KEY (`id`),
    UNIQUE KEY `id` (`id`)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE IF NOT EXISTS `block` (
    `id` int(10) unsigned NOT NULL AUTO_INCREMENT,
    `block` BIGINT(20) unsigned NOT NULL,
    `step` BIGINT(20) unsigned NOT NULL,
    PRIMARY KEY (`id`),
    UNIQUE KEY `id` (`id`)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE IF NOT EXISTS `coins` (
    `address` varchar(42) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT '',
    `symbol` varchar(10) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT '',
    `flag` tinyint(1) NOT NULL DEFAULT '0',
    PRIMARY KEY (`address`),
    UNIQUE KEY `address` (`address`)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE IF NOT EXISTS `price` (
    `proposal_id` int unsigned NOT NULL AUTO_INCREMENT,
    `ts` int NOT NULL DEFAULT 0,
    `token1` bigint NOT NULL DEFAULT '0',
    `token2` bigint NOT NULL DEFAULT '0',
    PRIMARY KEY (`proposal_id`, `ts`),
    UNIQUE KEY `proposal_id` (`proposal_id`, `ts`)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE IF NOT EXISTS `proposals` (
    `proposal_id` int unsigned NOT NULL AUTO_INCREMENT,
    `address` varchar(42) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT '',
    `token` varchar(42) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT '',
    `liquidity` bigint NOT NULL DEFAULT 0,
    `create_time` int NOT NULL DEFAULT 0,
    `close_time` int NOT NULL DEFAULT 0,
    `audit_state` enum('NotReviewed','Passed','NotPassed') CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT 'NotReviewed',
    `category` int NOT NULL DEFAULT 0,
    `state` enum('Original','Formal','End') CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT 'Original',
    `volume` bigint NOT NULL DEFAULT 0,
    `volume24` bigint NOT NULL DEFAULT 0,
    PRIMARY KEY (`proposal_id`),
    UNIQUE KEY `address` (`address`)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE IF NOT EXISTS `relations` (
    `proposal_id` int NOT NULL DEFAULT '0',
    `address` varchar(42) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT '',
    `relations` enum('Liquidity','Create','Trade') CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT '',
    PRIMARY KEY (`proposal_id`,`address`,`relations`) USING BTREE,
    UNIQUE KEY `proposal_id` (`proposal_id`,`address`,`relations`)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

```

## 说明

-   所有请求正确返回的格式

    ```json
    {
        "code": 200,
        "data": "非空对象"
    }
    ```

-   异常返回

    ```json
    {
        "code": 400,
        "message": "not permission"
    }
    ```

## Http

### banner

-   req

    ```http
    GET /banners
    ```

-   res

    ```json
    {
        "code": 200,
        "data": ["https://www.baidu.com", "https://www.baidu.com"]
    }
    ```

### 创建提案支持的币列表

-   req

    ```http
    GET /coins
    ```

-   res

    ```json
    {
        "code": 200,
        "data": [
            { "address": "0xabcd1234", "symbol": "xxxx" },
            { "address": "0xabcd1234", "symbol": "xxxx" }
        ]
    }
    ```

### 提案类型列表

-   req

    ```http
    GET /categories/{filter}
    ```

    -   **`filter`可选参数，传其他的会报错**

        -   `categories`

        -   `liquidity`

-   res

    ```json
    {
        "code": 200,
        "data": ["Cryptocurrency", "Politics"]
    }
    ```

### My Original prediction列表

-   req

    ```http
    GET /original/{audit_state}
    ```

    -   path params

        | 参数    | 描述 | 是否必须 |
        | :------ | :--- | :------- |
        | `audit_state` | 状态 | 是       |

        -   **`audit_state` 可选参数，传其他的会报错：**

            -   `not_reviewed`
            -   `passed`
            -   `not_passed`

    -   query params

        | 参数        | 描述                                                                                                                        | 是否必须                                            |
        | :---------- | :-------------------------------------------------------------------------------------------------------------------------- | :-------------------------------------------------- |
        | `page`      | 当前多少页                | 是          |
        | `count`     | 每页显示的数量            | 是          |
        | `aboutMe`   | 固定传1 - 我创建的      |是   |
        | `account`   | 当前账户的地址           | 是    |
        | `token`     | token 筛选                | 否      |

        

-   res

    ```json
    {
        "code": 200,
        "data": {
            "total": 1,
            "current": 1,
            "list": [
            {
                "proposalId": 2,
                "createTime": 1648547086,
                "address": "0x8baaaa876f1aed239729a3dcc0540f64094f9b75"
            },
            {
                "proposalId": 3,
                "createTime": 1648816799,
                "address": "0xb68a1a6209beea7bce11e1890fef2885c1eff630"
            }
            ]
        }
}
    ```

### Formal prediction列表

-   req

    ```http
    GET /formal/{status}
    ```

    -   path params

        | 参数    | 描述    | 是否必须 |
        | :------ | :------ | :------- |
        | `status`    | 提案状态 | 是       |

        -   **`state` 可选参数，传其他的会报错：**

            -   `formal`
            -   `end`

    -   query params

        | 参数      | 描述           | 是否必须 |
        | :-------- | :------------- | :------- |
        | `page`      | 当前多少页                | 是          |
        | `count`     | 每页显示的数量            | 是          |
        | `aboutMe`   | 固定传1 - 我创建的      |是   |
        | `account`   | 当前账户的地址           | 是    |
        | `token`     | token 筛选                | 否      |

-   res

    ```json
    {
        "code": 200,
        "data": {
            "total": 1,
            "current": 1,
            "list": [
            {
                "proposalId": 1,
                "createTime": 1648555200,
                "address": "0x06e506a77cf6e6e8c2e96ad29670e19ffa6ed778"
            },
            {
                "proposalId": 3,
                "createTime": 1648816799,
                "address": "0xb68a1a6209beea7bce11e1890fef2885c1eff630"
            }
            ]
        }
    }
    ```

### 历史

-   req

    ```http
    GET /history/{id}
    ```

    -   path params

        | 参数 | 描述    | 是否必须 |
        | :--- | :------ | :------- |
        | `id` | 提案 ID | 是       |


-   res

    ```json
    {"code":200,"data":[[1,"0.55","0.45"],[2,"0.45","0.55"],[3,"0.45","0.55"],[4,"0.45","0.55"],[5,"0.45","0.55"],[6,"0.45","0.55"],[7,"0.45","0.55"],[8,"0.45","0.55"],[9,"0.45","0.55"],[10,"0.45","0.55"],[11,"0.45","0.55"],[12,"0.45","0.55"],[13,"0.45","0.55"],[14,"0.45","0.55"],[15,"0.45","0.55"],[16,"0.45","0.55"],[17,"0.45","0.55"],[18,"0.45","0.55"],[19,"0.45","0.55"],[20,"0.45","0.55"],[21,"0.45","0.55"],[22,"0.45","0.55"],[23,"0.45","0.55"],[24,"0.45","0.55"],[25,"0.45","0.55"],[26,"0.45","0.55"],[27,"0.50","0.50"],[28,"0.50","0.50"],[29,"0.58","0.42"],[30,"0.74","0.26"]]}
    ```

### 后台查询提案

-   req

    ```http
    GET /backstage/{token}
    ```

    -   query params

        | 参数      | 描述                           | 是否必须 |
        | :-------- | :----------------------------- | :------- |
        | `count`   | 每页显示的数量                 | 是       |
        | `page`    | 当前多少页                     | 是       |

-   res

    ```json
    {
        "code": 200,
        "data": [
            3,  //总页数
            [   //当前页的提案id
                1,
                2,
                3,
                4,
                5,
                6
            ]
        ]
    }
    ```



## Websocket

-   websocket 连接路径 `/`
-   所有`sub`指令均支持`unsub`

### 心跳

-   十秒不发心跳就会自动关闭连接

-   req

    ```json
    {
        "op": "ping"
    }
    ```

-   res

    ```json
    {
        "op": "pong"
    }
    ```

### 订阅创建提案支持的币种列表

-   req

    ```json
    {
        "op": "sub",
        "target": "coinsSupport",
        "id": 123456
    }
    ```

-   res

    ```json
    {
        "code": 200,
        "id": 123456
    }
    ```

-   push

    ```json
    {
        "op": "<add|del>",
        "target": "coinsSupport",
        "data": "0xabcd1234",
        "id": 123456
    }
    ```

### 区块更新

-   req

    ```json
    {
        "op": "sub",
        "target": "newBlock",
        "id": 123456
    }
    ```

-   res

    ```json
    {
        "code": 200,
        "id": 123456
    }
    ```

-   push

    ```json
    {
        "op": "update",
        "target": "newBlock",
        "data": 10000,
        "id": 123456
    }
    ```

### 提案状态变化

-   req

    ```json
    {
        "op": "sub",
        "target": "proposalStatus",
        "id": 123456
    }
    ```

-   res

    ```json
    {
        "code": 200,
        "id": 123456
    }
    ```

-   push

    ```json
    {
        "op": "update",
        "target": "proposalStatus",
        "data": {
            "proposalId": 1,
            "address": "0x000000000",
            "state": 1
        },
        "id": 123456
    }
    ```
