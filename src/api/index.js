import axios from "axios";
import md5 from "md5";
let api={}

//const BASE_URL = "http://localhost:3000/"; // 本地
const BASE_URL = "https://eplayer-server.vercel.app/"; // 线上
api = axios.create({

    withCredentials: false, 
    baseURL:BASE_URL+'api'
})

// 添加请求拦截器设置认证头
// api.interceptors.request.use(config => {
//   const token = localStorage.getItem('token'); // 从localStorage获取token
//   if (token) {
//     config.headers.Authorization = `Bearer ${token}`;
//   }
//   return config;
// });

// 添加重试函数
async function retryOperation(operation, maxAttempts = 3, delay = 1000) {
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      return await operation();
    } catch (error) {
      // 如果是最后一次尝试，直接抛出错误
      if (attempt === maxAttempts) {
        throw error;
      }

      // 检查是否是网络错误或服务器错误
      if (error.response?.status >= 500 || error.code === 'ECONNABORTED' || error.code === 'ERR_NETWORK') {
        console.log(`尝试第 ${attempt} 次失败，${delay/1000}秒后重试...`, error);
        await new Promise(resolve => setTimeout(resolve, delay));
        // 每次重试增加延迟时间
        delay *= 2;
        continue;
      }

      // 其他类型的错误直接抛出
      throw error;
    }
  }
}

//user
export const getUser = (headers) => api.get(`/user/user`, { headers }) // success
export const getNotebook = (headers) => api.get(`/user/notebook`, { headers }) // success
export const updateUser = (payload, headers) => api.put(`/user/updateUser`,payload, { headers }) // success
//export const updateUserInfo = (payload) => api.post(`/user`,payload)
export const createUser = (payload) => api.post(`/user`,payload) // success
export const loginUser = (credentials) => api.get(`/user/login?username=${credentials.username}&password=${credentials.password}`) // success
export const updateUserVersion = (payload, headers) => api.put(`/user/updateVersion`, payload, { headers })  // success
export const deleteUser = (id) => api.delete(`/user/${id}`)

//subtitle
export const getSubtitle = (payload, headers) => api.get(`/subtitle/getSubtitle?md5=${payload.md5}`, { headers })  //叫id还是md5都一样  success
export const createSubtitle = (payload, headers) => api.post(`/subtitle`,payload, { headers})
export const deleteSubtitle = (id, headers) => api.delete(`/subtitle/${id}`, { headers })
export const updateSubtitle = (payload, headers) => api.put(`/subtitle/updateSubtitle?md5=${payload.md5}`,payload, { headers })  // success

const apis={
    getNotebook: async (headers) => {
        return await retryOperation(async () => {
            return await api.get(`/user/notebook`, { headers });
        });
    },
    
    getUser: async (headers) => {
        return await retryOperation(async () => {
            return await api.get(`/user/user`, { headers });
        });
    },
    
    updateUser: async (payload, headers) => {
        return await retryOperation(async () => {
            return await api.put(`/user/updateUser`, payload, { headers });
        });
    },
    
    createUser: async (payload) => {
        return await retryOperation(async () => {
            return await api.post(`/user`, payload);
        });
    },
    
    loginUser: async (credentials) => {
        return await retryOperation(async () => {
            return await api.get(`/user/login?username=${credentials.username}&password=${credentials.password}`);
        });
    },
    
    updateUserVersion: async (payload, headers) => {
        return await retryOperation(async () => {
            return await api.put(`/user/updateVersion`, payload, { headers });
        });
    },
    
    getSubtitle: async (payload, headers) => {
        return await retryOperation(async () => {
            return await api.get(`/subtitle/getSubtitle?md5=${payload.md5}`, { headers });
        });
    },
    
    createSubtitle: async (payload, headers) => {
        return await retryOperation(async () => {
            return await api.post(`/subtitle`, payload, { headers });
        });
    },
    
    updateSubtitle: async (payload, headers) => {
        return await retryOperation(async () => {
            return await api.put(`/subtitle/updateSubtitle?md5=${payload.md5}`, payload, { headers });
        });
    }
}
export default apis