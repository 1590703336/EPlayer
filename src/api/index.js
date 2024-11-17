import axios from "axios";
import md5 from "md5";
let api={}

const BASE_URL = "http://localhost:3000/"; // 本地
//const BASE_URL = "https://eplayer-server.vercel.app/"; // 线上
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
export const updateSubtitle = (payload, headers) => api.put(`/subtitle/updateSubtitle?md5=${payload.md5}`,payload, { headers })

const apis={
    getNotebook,
    getUser,
    updateUser,
    //updateUserInfo,
    createUser,
    loginUser,
    updateUserVersion,
    deleteUser,
    getSubtitle,
    createSubtitle,
    deleteSubtitle,
    updateSubtitle
}
export default apis